use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::iter;

use super::*;

#[cfg(target_os = "linux")]
#[test]
fn linux_permission_errors_explain_device_access_setup() {
    for error in [
        HidError::IoError {
            error: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        },
        HidError::HidApiError {
            message: String::from(
                "failed to open device with path /dev/hidraw4: Permission denied (os error 13)",
            ),
        },
    ] {
        let message = HidppError::Hid(error).to_string();
        assert!(message.contains("sudo /path/to/gbat"), "{message}");
    }
    let message = HidppError::Hid(HidError::HidApiError {
        message: String::from(
            "failed to open device with path /dev/hidraw4: No such file or directory (os error 2)",
        ),
    })
    .to_string();
    assert!(!message.contains("sudo /path/to/gbat"), "{message}");
}

const UNIFIED_LOOKUP: [u8; 7] = unified_lookup(1);
const LEGACY_LOOKUP: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x00, 0];
const UNIFIED_QUERY: [u8; 7] = [REPORT_ID_SHORT, 1, 5, 0x1F, 0, 0, 0];
const LEGACY_QUERY: [u8; 7] = [REPORT_ID_SHORT, 1, 6, 0x0F, 0, 0, 0];
const UNIFIED_FEATURE: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 5, 0, 0];
const LEGACY_FEATURE: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 6, 0, 0];
const LEGACY_STATUS: [u8; 7] = [REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0, 0x81];
const LEGACY_READING: Reading = Reading::Status(BatteryStatus {
    level: 42,
    charging: true,
});
/// A unified status reply that lacks the charging byte.
const INCOMPLETE_UNIFIED: [u8; 7] = [REPORT_ID_SHORT, 1, 5, 0x1F, 99, 0, 0];
const RECEIVER_OFFLINE: [u8; 7] = receiver_offline(1);

const fn unified_lookup(device_index: u8) -> [u8; 7] {
    [REPORT_ID_SHORT, device_index, 0, 0x0F, 0x10, 0x04, 0]
}

// Captured from a LIGHTSPEED receiver while its paired mouse was off.
const fn receiver_offline(device_index: u8) -> [u8; 7] {
    [REPORT_ID_SHORT, device_index, 0x8F, 0, 0x0F, 0x08, 0]
}

fn report(bytes: &[u8]) -> Report {
    let mut buffer = [0; REPORT_BUFFER_LEN];
    buffer[..bytes.len()].copy_from_slice(bytes);
    Report {
        buffer,
        length: bytes.len(),
    }
}

fn status(level: u8, charging: bool) -> Reading {
    Reading::Status(BatteryStatus { level, charging })
}

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// One expected I/O call, its result, and how far it advances the clock.
#[derive(Debug)]
enum ScriptStep {
    Read {
        timeout_ms: i32,
        result: Result<Vec<u8>, HidError>,
        advance: Duration,
    },
    Write {
        report: Vec<u8>,
        result: Result<(), HidError>,
        advance: Duration,
    },
}

impl ScriptStep {
    fn read(timeout_ms: i32, report: &[u8]) -> Self {
        Self::Read {
            timeout_ms,
            result: Ok(report.to_vec()),
            advance: Duration::ZERO,
        }
    }

    /// A nonblocking read while flushing before a request.
    fn flush(report: &[u8]) -> Self {
        Self::read(0, report)
    }

    fn write(report: &[u8]) -> Self {
        Self::Write {
            report: report.to_vec(),
            result: Ok(()),
            advance: Duration::ZERO,
        }
    }

    fn silence(timeout_ms: i32, advance: Duration) -> Self {
        Self::read(timeout_ms, &[]).after(advance)
    }

    fn after(mut self, elapsed: Duration) -> Self {
        let (Self::Read { advance, .. } | Self::Write { advance, .. }) = &mut self;
        *advance = elapsed;
        self
    }
}

/// A report read with the full response timeout.
fn reply(report: &[u8]) -> ScriptStep {
    ScriptStep::read(1_500, report)
}

/// The full response timeout passing without a report.
fn no_reply() -> ScriptStep {
    ScriptStep::silence(1_500, RESPONSE_TIMEOUT)
}

/// Flush an empty queue, write `request`, then read `replies`.
fn exchange(request: &[u8], replies: impl IntoIterator<Item = ScriptStep>) -> Vec<ScriptStep> {
    let mut steps = vec![ScriptStep::flush(&[]), ScriptStep::write(request)];
    steps.extend(replies);
    steps
}

/// Device index 1 reports the unified battery feature at index 5.
fn found_unified() -> Vec<ScriptStep> {
    exchange(&UNIFIED_LOOKUP, [reply(&UNIFIED_FEATURE)])
}

fn offline_probe(device_index: u8) -> Vec<ScriptStep> {
    exchange(
        &unified_lookup(device_index),
        [reply(&receiver_offline(device_index))],
    )
}

fn silent_probe(device_index: u8) -> Vec<ScriptStep> {
    exchange(&unified_lookup(device_index), [no_reply()])
}

/// Plays back an exact sequence of I/O calls on a virtual clock. Any call
/// the script does not expect fails the test, and so does any step still
/// unplayed when the script is dropped.
struct ScriptedIo<'a> {
    clock: &'a Cell<Instant>,
    steps: RefCell<VecDeque<ScriptStep>>,
}

impl ScriptedIo<'_> {
    fn next_step(&self, call: &str) -> ScriptStep {
        self.steps
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("script exhausted during {call}"))
    }
}

impl Drop for ScriptedIo<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let steps = self.steps.get_mut();
            assert!(steps.is_empty(), "unplayed script steps: {steps:?}");
        }
    }
}

impl RequestIo for ScriptedIo<'_> {
    fn write(&self, report: &[u8]) -> Result<(), HidError> {
        let ScriptStep::Write {
            report: expected,
            result,
            advance,
        } = self.next_step("write")
        else {
            panic!("unexpected write of {report:02X?}");
        };
        assert_eq!(report, expected, "unexpected request bytes");
        self.clock.set(self.now() + advance);
        result
    }

    fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
        let ScriptStep::Read {
            timeout_ms: expected,
            result,
            advance,
        } = self.next_step("read")
        else {
            panic!("unexpected read with a {timeout_ms} ms timeout");
        };
        assert_eq!(timeout_ms, expected, "unexpected read timeout");
        self.clock.set(self.now() + advance);
        let report = result?;
        buffer[..report.len()].copy_from_slice(&report);
        Ok(report.len())
    }

    fn now(&self) -> Instant {
        self.clock.get()
    }
}

/// A scripted HID interface: the I/O it plays once opened, or the error
/// that opening it fails with.
type Interface = Result<Vec<ScriptStep>, HidError>;

/// One HID interface that plays `parts` in order.
fn script(parts: impl IntoIterator<Item = Vec<ScriptStep>>) -> Interface {
    Ok(parts.into_iter().flatten().collect())
}

/// Read the battery through scripted HID interfaces on one virtual clock,
/// returning the outcome and the virtual time it took.
fn read_scripted(
    interfaces: impl IntoIterator<Item = Interface>,
) -> (Result<Reading, HidppError>, Duration) {
    let start = Instant::now();
    let clock = Cell::new(start);
    let openers = interfaces.into_iter().map(|interface| {
        // Keep the script's drop check active even if its opener is skipped.
        let scripted = interface.map(|steps| ScriptedIo {
            clock: &clock,
            steps: RefCell::new(steps.into()),
        });
        move || scripted
    });
    let result = read_battery_from(openers, || clock.get());
    (result, clock.get() - start)
}

/// Read the battery through one scripted HID interface.
fn read_one(
    parts: impl IntoIterator<Item = Vec<ScriptStep>>,
) -> (Result<Reading, HidppError>, Duration) {
    read_scripted([script(parts)])
}

fn assert_short_response<T: fmt::Debug>(
    result: Result<T, HidppError>,
    minimum: usize,
    actual: usize,
) {
    let Err(error @ HidppError::ShortResponse { .. }) = result else {
        panic!("expected a short response error, got {result:?}");
    };
    assert_eq!(
        error.to_string(),
        format!("short HID++ response: expected at least {minimum} bytes, got {actual}")
    );
}

#[test]
fn classifies_reports_against_the_request() {
    // The request goes to device index 1, feature 5, function 1, software ID 0x0F.
    let short_reply = [REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0];
    let long_reply = [REPORT_ID_LONG, 1, 5, 0x1F, 78, 0, 0, 1];
    let cases: [(&[u8], Option<Reply>); 14] = [
        (&short_reply, Some(Reply::Response(report(&short_reply)))),
        (&long_reply, Some(Reply::Response(report(&long_reply)))),
        (&[0x12, 1, 5, 0x1F, 78, 0, 0], None),
        (&[REPORT_ID_SHORT, 2, 5, 0x1F, 78, 0, 0], None),
        (&[REPORT_ID_SHORT, 1, 6, 0x1F, 78, 0, 0], None),
        (&[REPORT_ID_SHORT, 1, 5, 0x0F, 78, 0, 0], None),
        (&[REPORT_ID_SHORT, 1, 5, 0x1E, 78, 0, 0], None),
        (&[REPORT_ID_SHORT, 1], None),
        (
            &[REPORT_ID_SHORT, 1, 0x8F, 5, 0x1F, 0x08, 0],
            Some(Reply::Offline),
        ),
        (
            &[REPORT_ID_SHORT, 1, 0x8F, 5, 0x1F, 0x02, 0],
            Some(Reply::DeviceError),
        ),
        // Only the receiver's HID++ 1.0 error means no connected device.
        (
            &[REPORT_ID_LONG, 1, 0xFF, 5, 0x1F, 0x08, 0],
            Some(Reply::DeviceError),
        ),
        // Errors must echo the request too.
        (&[REPORT_ID_SHORT, 1, 0x8F, 6, 0x1F, 0x08, 0], None),
        (&[REPORT_ID_SHORT, 1, 0xFF, 5, 0x0F, 0x02, 0], None),
        (&[REPORT_ID_SHORT, 1, 0x8E, 5, 0x1F, 0x02, 0], None),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            match_report(report(bytes), 1, 5, 0x1F),
            expected,
            "report {bytes:02X?}"
        );
    }
}

mod discovery {
    use super::*;

    #[test]
    fn skips_unrelated_reports_and_errors_until_the_matching_reply() {
        let (result, _) = read_one([
            exchange(
                &UNIFIED_LOOKUP,
                [
                    reply(&[REPORT_ID_SHORT, 2, 0, 0x0F, 9, 0, 0]),
                    reply(&[REPORT_ID_SHORT, 1, 0, 0x1F, 9, 0, 0]),
                    reply(&[REPORT_ID_SHORT, 1, 0x8F, 0, 0x0E, 0x08, 0]),
                    reply(&UNIFIED_FEATURE),
                ],
            ),
            exchange(
                &UNIFIED_QUERY,
                [
                    reply(&[REPORT_ID_LONG, 1, 5, 0x0F, 12, 0, 0, 0]),
                    reply(&[REPORT_ID_SHORT, 1, 0xFF, 6, 0x1F, 0x02, 0]),
                    reply(&[REPORT_ID_LONG, 1, 5, 0x1F, 78, 0, 0, 1]),
                ],
            ),
        ]);

        assert_eq!(result.unwrap(), status(78, true));
    }

    #[test]
    fn reads_long_reports_after_flushing_stale_ones() {
        for (device_index, feature_index, level, flags, charging) in
            [(1, 5, 78, 0x81, true), (0xFF, 9, 42, 0x02, false)]
        {
            let long_report = |level, flags| {
                let mut report = [0; REPORT_BUFFER_LEN];
                report[..8].copy_from_slice(&[
                    REPORT_ID_LONG,
                    device_index,
                    feature_index,
                    0x1F,
                    level,
                    0,
                    0,
                    flags,
                ]);
                report
            };
            // Direct USB is only probed after the receiver slot.
            let earlier_probes = if device_index == 1 {
                vec![]
            } else {
                offline_probe(1)
            };
            let (result, _) = read_one([
                earlier_probes,
                vec![ScriptStep::flush(&[
                    REPORT_ID_SHORT,
                    device_index,
                    0,
                    0x0F,
                    99,
                    0,
                    0,
                ])],
                exchange(
                    &unified_lookup(device_index),
                    [reply(&[
                        REPORT_ID_SHORT,
                        device_index,
                        0,
                        0x0F,
                        feature_index,
                        0,
                        0,
                    ])],
                ),
                vec![ScriptStep::flush(&long_report(8, 0))],
                exchange(
                    &[REPORT_ID_SHORT, device_index, feature_index, 0x1F, 0, 0, 0],
                    [reply(&long_report(level, flags))],
                ),
            ]);

            assert_eq!(result.unwrap(), status(level, charging));
        }
    }

    #[test]
    fn an_unsupported_unified_feature_selects_and_reads_legacy() {
        for unified_reply in [
            [REPORT_ID_SHORT, 1, 0, 0x0F, 0, 0, 0],
            [REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP10, 0, 0x0F, 0x02, 0],
            [REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP20, 0, 0x0F, 0x02, 0],
            [REPORT_ID_LONG, 1, ERROR_MARKER_HIDPP20, 0, 0x0F, 0x08, 0],
        ] {
            // Reading the selected legacy feature skips the unified query.
            let (result, _) = read_one([
                exchange(&UNIFIED_LOOKUP, [reply(&unified_reply)]),
                exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE)]),
                exchange(&LEGACY_QUERY, [reply(&LEGACY_STATUS)]),
            ]);

            assert_eq!(result.unwrap(), LEGACY_READING);
        }
    }

    #[test]
    fn an_offline_receiver_ends_each_probe_without_waiting_or_legacy_lookup() {
        let reply_delay = ms(17);
        let expected_indices = [1, 0xFF, 2, 3, 4, 5, 6];
        let (result, elapsed) = read_one(expected_indices.map(|device_index| {
            exchange(
                &unified_lookup(device_index),
                [reply(&receiver_offline(device_index)).after(reply_delay)],
            )
        }));

        assert!(matches!(result, Ok(Reading::Offline)));
        assert_eq!(elapsed, reply_delay * expected_indices.len() as u32);
    }

    #[test]
    fn checks_likely_indices_on_every_interface_first() {
        let (result, _) = read_scripted([
            script([offline_probe(1), offline_probe(0xFF)]),
            script([
                offline_probe(1),
                exchange(
                    &unified_lookup(0xFF),
                    [reply(&[REPORT_ID_SHORT, 0xFF, 0, 0x0F, 9, 0, 0])],
                ),
                exchange(
                    &[REPORT_ID_SHORT, 0xFF, 9, 0x1F, 0, 0, 0],
                    [reply(&[REPORT_ID_LONG, 0xFF, 9, 0x1F, 53, 0, 0, 1])],
                ),
            ]),
        ]);

        assert_eq!(result.unwrap(), status(53, true));
    }

    #[test]
    fn offline_outranks_later_errors_while_preserving_candidate_order() {
        for offline_first in [true, false] {
            let (result, _) = read_one(DEVICE_INDEX_CANDIDATES.map(|device_index| {
                let truncated = [REPORT_ID_SHORT, device_index, 0, 0x0F];
                let answer = if offline_first && device_index == 1 {
                    reply(&RECEIVER_OFFLINE)
                } else {
                    reply(&truncated)
                };
                exchange(&unified_lookup(device_index), [answer])
            }));

            if offline_first {
                assert!(matches!(result, Ok(Reading::Offline)));
            } else {
                assert_short_response(result, 5, 4);
            }
        }
    }

    #[test]
    fn an_interface_that_fails_to_open_is_skipped() {
        let unopenable = || Err(HidError::InitializationError);

        // A later HID interface that finds the battery hides the open error.
        let (result, _) = read_scripted([
            unopenable(),
            script([
                found_unified(),
                exchange(
                    &UNIFIED_QUERY,
                    [reply(&[REPORT_ID_LONG, 1, 5, 0x1F, 78, 0, 0, 1])],
                ),
            ]),
        ]);
        assert_eq!(result.unwrap(), status(78, true));

        // So does an offline receiver.
        let (result, _) = read_scripted([
            unopenable(),
            script(DEVICE_INDEX_CANDIDATES.map(offline_probe)),
        ]);
        assert!(matches!(result, Ok(Reading::Offline)));

        // Otherwise the open error explains the failure.
        let (result, _) = read_scripted([unopenable(), script([1, 0xFF, 2, 3].map(silent_probe))]);
        assert!(matches!(
            result,
            Err(HidppError::Hid(HidError::InitializationError))
        ));
    }
}

mod request_timing {
    use super::*;

    #[test]
    fn continuous_queued_reports_stop_flushing_at_the_window_boundary() {
        for last_read_ms in [1, 2] {
            let stale = [REPORT_ID_SHORT, 1, 5, 0x1F, 8, 0, 0, 0];
            let (result, elapsed) = read_one([
                found_unified(),
                vec![
                    ScriptStep::flush(&stale).after(ms(100)),
                    ScriptStep::flush(&stale).after(ms(99)),
                    ScriptStep::flush(&stale).after(ms(last_read_ms)),
                    ScriptStep::write(&UNIFIED_QUERY),
                    reply(&[REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0, 1]),
                ],
            ]);

            assert_eq!(result.unwrap(), status(78, true));
            assert_eq!(elapsed, ms(199 + last_read_ms));
        }
    }

    #[test]
    fn unrelated_reports_consume_the_original_response_wait() {
        let (result, elapsed) = read_one(
            iter::once(exchange(
                &UNIFIED_LOOKUP,
                [
                    ScriptStep::read(1_500, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]).after(ms(500)),
                    ScriptStep::read(1_000, &[REPORT_ID_SHORT, 1, 0, 0x0E, 5, 0, 0]).after(ms(750)),
                    ScriptStep::silence(250, ms(250)),
                ],
            ))
            .chain(
                DEVICE_INDEX_CANDIDATES[1..]
                    .iter()
                    .copied()
                    .map(offline_probe),
            ),
        );

        assert!(matches!(result, Ok(Reading::Offline)));
        assert_eq!(elapsed, RESPONSE_TIMEOUT);
    }

    #[test]
    fn fractional_milliseconds_are_truncated_without_ending_the_wait_early() {
        let unrelated = [REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0];
        let remaining = Duration::from_micros(2_500);
        let (result, elapsed) = read_one([
            // The receiver answers so late that only 2.5 ms of the budget remain.
            exchange(
                &UNIFIED_LOOKUP,
                [reply(&RECEIVER_OFFLINE).after(DISCOVERY_BUDGET - remaining)],
            ),
            exchange(
                &unified_lookup(0xFF),
                [
                    ScriptStep::read(2, &unrelated).after(Duration::from_micros(1_750)),
                    ScriptStep::read(0, &unrelated).after(Duration::from_micros(500)),
                    ScriptStep::silence(0, Duration::from_micros(250)),
                ],
            ),
        ]);

        assert!(matches!(result, Ok(Reading::Offline)));
        assert_eq!(elapsed, DISCOVERY_BUDGET);
    }

    #[test]
    fn a_lookup_with_no_budget_left_ignores_an_unrelated_report() {
        let (result, _) = read_one([
            exchange(
                &UNIFIED_LOOKUP,
                [reply(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0, 0, 0]).after(DISCOVERY_BUDGET)],
            ),
            exchange(
                &LEGACY_LOOKUP,
                [ScriptStep::read(0, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0])],
            ),
        ]);

        assert!(matches!(result, Ok(Reading::NotFound)));
    }

    #[test]
    fn discovery_overhead_preserves_the_wait_and_later_battery_request_durations() {
        let (result, elapsed) = read_scripted([
            // Only 400 ms of the budget remain when the next HID interface
            // is probed.
            script([exchange(
                &UNIFIED_LOOKUP,
                [reply(&RECEIVER_OFFLINE).after(ms(5_600))],
            )]),
            script([
                vec![
                    ScriptStep::flush(&[REPORT_ID_SHORT, 1, 0, 0x0F, 99, 0, 0]).after(ms(150)),
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&UNIFIED_LOOKUP).after(ms(100)),
                    ScriptStep::read(400, &UNIFIED_FEATURE).after(ms(400)),
                ],
                exchange(&UNIFIED_QUERY, [no_reply()]),
                exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE).after(ms(1_500))]),
                exchange(&LEGACY_QUERY, [reply(&LEGACY_STATUS).after(ms(1_500))]),
            ]),
        ]);

        // A matching reply at the response deadline is accepted, and the
        // battery requests after it still get their full timeouts.
        assert_eq!(result.unwrap(), LEGACY_READING);
        assert_eq!(elapsed, ms(5_600 + 650 + 3 * 1_500));
    }

    #[test]
    fn silence_skips_the_legacy_lookup_and_the_budget_ends_discovery() {
        let (result, elapsed) = read_one([
            exchange(
                &UNIFIED_LOOKUP,
                [reply(&[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]), no_reply()],
            ),
            silent_probe(0xFF),
            silent_probe(2),
            silent_probe(3),
        ]);

        assert!(matches!(result, Ok(Reading::NotFound)));
        assert_eq!(elapsed, DISCOVERY_BUDGET);
    }

    #[test]
    fn a_probe_can_finish_its_legacy_lookup_after_the_discovery_budget_is_used() {
        let (result, elapsed) = read_one([
            silent_probe(1),
            silent_probe(0xFF),
            silent_probe(2),
            exchange(
                &unified_lookup(3),
                [reply(&[REPORT_ID_SHORT, 3, 0, 0x0F, 0, 0, 0]).after(ms(1_500))],
            ),
            exchange(
                &[REPORT_ID_SHORT, 3, 0, 0x0F, 0x10, 0x00, 0],
                [ScriptStep::read(0, &[REPORT_ID_SHORT, 3, 0, 0x0F, 6, 0, 0])],
            ),
            exchange(
                &[REPORT_ID_SHORT, 3, 6, 0x0F, 0, 0, 0],
                [reply(&[REPORT_ID_SHORT, 3, 6, 0x0F, 42, 0, 0x81])],
            ),
        ]);

        assert_eq!(result.unwrap(), LEGACY_READING);
        assert_eq!(elapsed, DISCOVERY_BUDGET);
    }
}

mod battery_recovery {
    use super::*;

    /// Find the unified feature on device index 1, then read it with `parts`.
    fn read_unified(
        parts: impl IntoIterator<Item = Vec<ScriptStep>>,
    ) -> Result<Reading, HidppError> {
        read_one(iter::once(found_unified()).chain(parts)).0
    }

    /// Look up the legacy feature, then read its status with `status_reply`.
    fn legacy_fallback(status_reply: ScriptStep) -> Vec<ScriptStep> {
        let mut steps = exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE)]);
        steps.extend(exchange(&LEGACY_QUERY, [status_reply]));
        steps
    }

    #[test]
    fn incomplete_unified_replies_recover_with_a_legacy_status() {
        // Every matching prefix still lacks the unified charging byte.
        for length in 4..=INCOMPLETE_UNIFIED.len() {
            let result = read_unified([
                exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED[..length])]),
                legacy_fallback(reply(&LEGACY_STATUS)),
            ]);

            assert_eq!(result.unwrap(), LEGACY_READING);
        }
    }

    #[test]
    fn offline_device_error_and_silence_recover_with_a_legacy_status() {
        for unified_reply in [
            reply(&[REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP10, 5, 0x1F, 0x08, 0]),
            reply(&[REPORT_ID_LONG, 1, ERROR_MARKER_HIDPP20, 5, 0x1F, 0x02]),
            no_reply(),
        ] {
            let result = read_unified([
                exchange(&UNIFIED_QUERY, [unified_reply]),
                legacy_fallback(reply(&LEGACY_STATUS)),
            ]);

            assert_eq!(result.unwrap(), LEGACY_READING);
        }
    }

    #[test]
    fn missing_results_from_both_features_return_no_battery_status() {
        let result = read_unified([
            exchange(&UNIFIED_QUERY, [no_reply()]),
            legacy_fallback(reply(&[
                REPORT_ID_SHORT,
                1,
                ERROR_MARKER_HIDPP10,
                6,
                0x0F,
                0x08,
                0,
            ])),
        ]);

        assert_eq!(result.unwrap(), Reading::NoStatus);
    }

    #[test]
    fn a_failed_legacy_lookup_preserves_the_unified_short_response() {
        for lookup_reply in [
            reply(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0, 0, 0]),
            reply(&[REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP10, 0, 0x0F, 0x08, 0]),
            reply(&[REPORT_ID_LONG, 1, ERROR_MARKER_HIDPP20, 0, 0x0F, 0x02]),
            reply(&[REPORT_ID_SHORT, 1, 0, 0x0F]),
            no_reply(),
        ] {
            let result = read_unified([
                exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                exchange(&LEGACY_LOOKUP, [lookup_reply]),
            ]);

            assert_short_response(result, 8, 7);
        }
    }

    #[test]
    fn a_failed_legacy_status_preserves_the_unified_short_response() {
        for status_reply in [
            reply(&[REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0]),
            reply(&[REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP10, 6, 0x0F, 0x08, 0]),
            reply(&[REPORT_ID_LONG, 1, ERROR_MARKER_HIDPP20, 6, 0x0F, 0x02]),
            no_reply(),
        ] {
            let result = read_unified([
                exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                legacy_fallback(status_reply),
            ]);

            assert_short_response(result, 8, 7);
        }
    }

    #[test]
    fn a_missing_unified_result_does_not_hide_a_legacy_short_response() {
        let result = read_unified([
            exchange(&UNIFIED_QUERY, [no_reply()]),
            legacy_fallback(reply(&[REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0])),
        ]);

        assert_short_response(result, 7, 6);
    }

    #[derive(Debug, Clone, Copy)]
    enum IoFailure {
        FlushRead,
        Write,
        ResponseRead,
    }

    impl IoFailure {
        const ALL: [Self; 3] = [Self::FlushRead, Self::Write, Self::ResponseRead];

        /// The script for `request` up to and including this failure.
        fn steps(self, request: &[u8]) -> Vec<ScriptStep> {
            let failed_read = |timeout_ms| ScriptStep::Read {
                timeout_ms,
                result: Err(HidError::InitializationError),
                advance: Duration::ZERO,
            };
            match self {
                Self::FlushRead => vec![failed_read(0)],
                Self::Write => vec![
                    ScriptStep::flush(&[]),
                    ScriptStep::Write {
                        report: request.to_vec(),
                        result: Err(HidError::InitializationError),
                        advance: Duration::ZERO,
                    },
                ],
                Self::ResponseRead => exchange(request, [failed_read(1_500)]),
            }
        }

        fn assert_error(self, result: Result<Reading, HidppError>) {
            assert!(
                matches!(result, Err(HidppError::Hid(HidError::InitializationError))),
                "{self:?} produced {result:?}"
            );
        }
    }

    // The scripts end at the failure, so these also check that a failed
    // read ends discovery instead of probing the next device index.
    #[test]
    fn unified_io_failures_stop_without_legacy_recovery() {
        for failure in IoFailure::ALL {
            failure.assert_error(read_unified([failure.steps(&UNIFIED_QUERY)]));
        }
    }

    #[test]
    fn legacy_io_failures_replace_the_unified_short_response() {
        for failure in IoFailure::ALL {
            failure.assert_error(read_unified([
                exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                failure.steps(&LEGACY_LOOKUP),
            ]));

            failure.assert_error(read_unified([
                exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE)]),
                failure.steps(&LEGACY_QUERY),
            ]));
        }
    }
}
