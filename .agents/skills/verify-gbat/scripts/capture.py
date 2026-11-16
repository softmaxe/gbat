#!/usr/bin/env -S uv run --no-project --python 3.12
"""Capture a short-lived real command in a private PTY and retain its proof."""

from __future__ import annotations

import argparse
import errno
import hashlib
import json
import os
import platform
import pty
import re
import select
import signal
import sys
import time
import tomllib
from datetime import UTC, datetime
from pathlib import Path
from types import FrameType


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(directory: Path) -> dict[str, str]:
    return {
        str(path.relative_to(directory)): (
            f"symlink:{os.readlink(path)}" if path.is_symlink() else digest(path)
        )
        for path in sorted(directory.rglob("*"))
        if path.is_symlink() or path.is_file()
    }


def write_json(path: Path, data: object) -> None:
    path.write_text(json.dumps(data, indent=2) + "\n")


def interrupt_handler(signum: int, frame: FrameType | None) -> None:
    raise KeyboardInterrupt


def capture(
    command: list[str], cwd: Path, evidence: Path, timeout: float
) -> tuple[int, bool, bool]:
    started = time.monotonic()
    status: int | None = None
    timed_out = False
    interrupted = False
    eof = False
    with (evidence / "stderr.txt").open("xb") as errors:
        sys.stdout.flush()
        sys.stderr.flush()
        pid, master = pty.fork()
        if pid == 0:
            try:
                os.chdir(cwd)
                os.dup2(errors.fileno(), 2)
                os.execvpe(command[0], command, os.environ)
            except OSError as error:
                os.write(2, f"Could not execute command: {error}\n".encode())
                os._exit(127)
        try:
            with (
                (evidence / "terminal.bin").open("xb") as terminal,
                (evidence / "events.jsonl").open("x") as events,
            ):
                while not (eof and status is not None):
                    if time.monotonic() - started >= timeout:
                        timed_out = True
                        break
                    if not eof and select.select([master], [], [], 0.1)[0]:
                        try:
                            chunk = os.read(master, 65536)
                        except OSError as error:
                            if error.errno != errno.EIO:
                                raise
                            chunk = b""
                        if chunk:
                            terminal.write(chunk)
                            terminal.flush()
                            events.write(
                                json.dumps(
                                    {
                                        "elapsed_seconds": time.monotonic() - started,
                                        "stream": "stdout-pty",
                                        "text": chunk.decode(errors="replace"),
                                    }
                                )
                                + "\n"
                            )
                            events.flush()
                        else:
                            eof = True
                    elif eof:
                        time.sleep(0.01)
                    if status is None:
                        waited, child_status = os.waitpid(pid, os.WNOHANG)
                        if waited:
                            status = child_status
        except KeyboardInterrupt:
            interrupted = True
        finally:
            try:
                # Closing the master first can invalidate killpg on macOS.
                if status is None or timed_out or interrupted:
                    try:
                        os.killpg(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if status is None:
                    _, status = os.waitpid(pid, 0)
            finally:
                os.close(master)
    return os.waitstatus_to_exitcode(status), timed_out, interrupted


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--cwd", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--feature", required=True)
    parser.add_argument(
        "--expect",
        choices=["version", "battery", "error", "missing-binary"],
        required=True,
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command: list[str] = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command or args.timeout <= 0:
        parser.error("Provide a command after -- and a positive timeout")
    cwd = args.cwd.resolve(strict=True)
    binary = args.binary.resolve(strict=True)
    evidence = args.evidence.resolve()
    if evidence == cwd or cwd in evidence.parents:
        parser.error("Evidence must be outside the scratch working directory")
    repo = Path(__file__).resolve().parents[4]
    version = tomllib.loads((repo / "Cargo.toml").read_text())["package"]["version"]
    evidence.mkdir(parents=True, exist_ok=False)
    before = snapshot(cwd)
    write_json(
        evidence / "action.json",
        {
            "feature": args.feature,
            "command": command,
            "cwd": str(cwd),
            "started_utc": datetime.now(UTC).isoformat(),
            "platform": platform.platform(),
            "binary": str(binary),
            "binary_sha256": digest(binary),
            "manifest_version": version,
            "expectation": args.expect,
            "timeout_seconds": args.timeout,
            "stderr_capture": "redirected to stderr.txt; stdout and stdin use a private PTY",
            "scratch_before": before,
        },
    )
    capture_error: str | None = None
    previous_handler = signal.signal(signal.SIGTERM, interrupt_handler)
    try:
        exit_code, timed_out, interrupted = capture(
            command, cwd, evidence, args.timeout
        )
    except (OSError, ValueError) as error:
        exit_code, timed_out, interrupted = -1, False, False
        capture_error = f"{type(error).__name__}: {error}"
    finally:
        signal.signal(signal.SIGTERM, previous_handler)
    for name in ("terminal.bin", "events.jsonl", "stderr.txt"):
        (evidence / name).touch(exist_ok=True)
    stdout = (evidence / "terminal.bin").read_bytes().replace(b"\r\n", b"\n")
    stderr = (evidence / "stderr.txt").read_bytes()
    (evidence / "stdout.txt").write_bytes(stdout)
    if args.expect == "version":
        matched = (
            exit_code == 0 and stdout == f"gbat {version}\n".encode() and not stderr
        )
    elif args.expect == "battery":
        matched = (
            exit_code == 0
            and re.fullmatch(
                rb"Battery: (?:100|[1-9]?[0-9])%(?: \(charging\))?\n", stdout
            )
            is not None
            and not stderr
        )
    elif args.expect == "missing-binary":
        matched = (
            exit_code == 1
            and not stdout
            and stderr
            == (b"gbat binary not found. Build it with: cargo build --release\n")
        )
    else:
        matched = (
            exit_code == 1
            and not stdout
            and stderr.startswith(
                (
                    b"No responsive Logitech HID++ interface found.",
                    b"The receiver reports no connected mouse.",
                    b"Could not read battery level",
                    b"Could not initialize HID access:",
                )
            )
        )
    after = snapshot(cwd)
    passed = bool(
        matched
        and not timed_out
        and not interrupted
        and not capture_error
        and before == after
    )
    result = {
        "feature": args.feature,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "interrupted": interrupted,
        "capture_error": capture_error,
        "expectation_matched": bool(matched),
        "scratch_unchanged": before == after,
        "scratch_after": after,
        "passed": passed,
    }
    write_json(evidence / "result.json", result)
    sys.stdout.buffer.write(stdout)
    sys.stdout.buffer.flush()
    sys.stderr.buffer.write(stderr)
    sys.stderr.buffer.flush()
    print(json.dumps({"evidence": str(evidence), **result}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
