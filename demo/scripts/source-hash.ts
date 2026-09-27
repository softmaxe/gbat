import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { ROOT } from "./paths";

/** Keep film checks from passing against outputs built before a source edit. */
export function sourceHash(): string {
  const hash = createHash("sha256");
  const ignored = new Set(["node_modules", "build", ".venv", "__pycache__", ".pytest_cache"]);
  function visit(directory: string): void {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (ignored.has(entry.name) || entry.name.endsWith(".egg-info") || entry.name === ".DS_Store") continue;
      const location = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(location);
      else if (entry.isFile()) hash.update(path.relative(ROOT, location)).update(fs.readFileSync(location));
    }
  }
  visit(ROOT);
  return hash.digest("hex");
}
