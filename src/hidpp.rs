use std::fmt;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice, HidError};

const LOGITECH_VID: u16 = 0x046D;
const HIDPP_USAGE_PAGE: u16 = 0xFF00;

const FEATURE_ROOT: u8 = 0x00;
const FEATURE_BATTERY_STATUS: u16 = 0x1000;
const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;

const REPORT_ID_SHORT: u8 = 0x10;
const REPORT_ID_LONG: u8 = 0x11;
/// Error markers in the third byte of a short or long report: receivers use
/// the HID++ 1.0 marker and HID++ 2.0 devices use their own.
const ERROR_MARKER_HIDPP10: u8 = 0x8F;
const ERROR_MARKER_HIDPP20: u8 = 0xFF;
/// HID++ 1.0 error code a receiver returns for a device index with no
/// connected device.
const ERROR_CODE_UNKNOWN_DEVICE: u8 = 0x08;
const SW_ID: u8 = 0x0F;

/// Receiver-paired devices answer on 1..=6 and a directly connected device on
/// 0xFF, so probe both common slots before the rarely used ones.
const DEVICE_INDEX_CANDIDATES: [u8; 7] = [1, 0xFF, 2, 3, 4, 5, 6];

const REPORT_BUFFER_LEN: usize = 20;
const FLUSH_WINDOW: Duration = Duration::from_millis(200);
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(1_500);
/// Upper bound for probing every interface and device index. A paired but
/// unavailable index answers with an error report instead of falling silent, so
/// this budget only caps interfaces that never answer at all.
const DISCOVERY_BUDGET: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    pub level: u8,
    pub charging: bool,
}

#[derive(Debug)]
pub enum HidppError {
    Hid(HidError),
    /// A reply matched the request but is too short to decode. `match_report`
    /// already checked the report ID, device index, feature index, and function
    /// byte, and turned error reports into their own outcomes.
    ShortResponse {
        minimum: usize,
        actual: usize,
    },
}

impl fmt::Display for HidppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hid(error) => write!(formatter, "HID access failed: {error}"),
            Self::ShortResponse { minimum, actual } => write!(
                formatter,
                "short HID++ response: expected at least {minimum} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for HidppError {}

impl From<HidError> for HidppError {
    fn from(error: HidError) -> Self {
        Self::Hid(error)
    }
}

/// One report read from a HID interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Report {
    buffer: [u8; REPORT_BUFFER_LEN],
    length: usize,
}

impl Report {
    fn bytes(&self) -> &[u8] {
        &self.buffer[..self.length]
    }
}

/// What a single HID++ request produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Response(Report),
    /// The device index exists but rejected the request.
    DeviceError,
    /// The receiver has no connected device at this index.
    Offline,
    /// Nothing answered within the timeout.
    Silence,
}

/// Outcome of a root-feature lookup.
enum FeatureLookup {
    Index(u8),
    Unsupported,
    Offline,
    Silent,
}

/// What probing one device index found.
enum Discovery {
    Found(BatteryFeature),
    /// A receiver answered, but reported no connected mouse.
    Offline,
    NotFound,
}

/// What reading the battery through every candidate HID interface found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Status(BatteryStatus),
    /// A device index exposed a battery feature, but reading it produced no
    /// status.
    NoStatus,
    /// A receiver answered, but reported no connected mouse.
    Offline,
    /// No HID interface answered at any device index.
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatteryFeature {
    Unified(u8),
    Legacy(u8),
}

/// Report I/O and the monotonic timeline used by a request.
trait RequestIo {
    fn write(&self, report: &[u8]) -> Result<(), HidError>;
    fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, HidError>;
    fn now(&self) -> Instant;
}

impl RequestIo for HidDevice {
    // hidapi's macOS backend writes a report whole or fails, so the returned
    // byte count carries no information.
    fn write(&self, report: &[u8]) -> Result<(), HidError> {
        HidDevice::write(self, report).map(drop)
    }

    fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
        HidDevice::read_timeout(self, buffer, timeout_ms)
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

fn ensure_length(response: &[u8], minimum: usize) -> Result<(), HidppError> {
    if response.len() < minimum {
        return Err(HidppError::ShortResponse {
            minimum,
            actual: response.len(),
        });
    }
    Ok(())
}

fn parse_feature_index(response: &[u8]) -> Result<Option<u8>, HidppError> {
    ensure_length(response, 5)?;
    let feature_index = response[4];
    Ok((feature_index != 0).then_some(feature_index))
}

/// Both battery features report the level in byte 4; only the position of the
/// charging flag differs.
fn parse_battery(response: &[u8], charging_offset: usize) -> Result<BatteryStatus, HidppError> {
    ensure_length(response, charging_offset + 1)?;
    Ok(BatteryStatus {
        level: response[4],
        charging: response[charging_offset] & 0x01 != 0,
    })
}

/// Discard reports that arrived before the current request. Queued reports are
/// already available, so this never waits for new ones.
fn flush(io: &impl RequestIo) -> Result<(), HidppError> {
    let deadline = io.now() + FLUSH_WINDOW;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    while io.now() < deadline {
        if io.read_timeout(&mut buffer, 0)? == 0 {
            break;
        }
    }
    Ok(())
}

/// Remaining time for one request, capped at the per-request timeout.
fn request_timeout(io: &impl RequestIo, deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(io.now())
        .min(RESPONSE_TIMEOUT)
}

/// The reply `report` gives to the request, or `None` if it answers something
/// else. A HID++ 2.0 reply echoes the request's function and software ID, so
/// matching that byte too rejects stale reports and replies meant for another
/// reader. An error arrives as an ordinary short or long report whose third
/// byte is the error marker, followed by the request's feature index, function
/// byte, and the error code.
fn match_report(
    report: Report,
    device_index: u8,
    feature_index: u8,
    function_byte: u8,
) -> Option<Reply> {
    let bytes = report.bytes();
    let header_matches = matches!(
        bytes.first(),
        Some(&REPORT_ID_SHORT) | Some(&REPORT_ID_LONG)
    ) && bytes.get(1) == Some(&device_index);
    if !header_matches {
        return None;
    }
    match bytes[2..] {
        [feature, function, ..] if feature == feature_index && function == function_byte => {
            Some(Reply::Response(report))
        }
        [marker @ (ERROR_MARKER_HIDPP10 | ERROR_MARKER_HIDPP20), feature, function, code, ..]
            if feature == feature_index && function == function_byte =>
        {
            if marker == ERROR_MARKER_HIDPP10 && code == ERROR_CODE_UNKNOWN_DEVICE {
                Some(Reply::Offline)
            } else {
                Some(Reply::DeviceError)
            }
        }
        _ => None,
    }
}

/// The device at one device index, reached through an open HID interface.
struct Device<'io, Io> {
    io: &'io Io,
    index: u8,
}

impl<Io: RequestIo> Device<'_, Io> {
    /// Send one short request and wait for its reply.
    fn request(
        &self,
        feature_index: u8,
        function: u8,
        payload: [u8; 3],
        timeout: Duration,
    ) -> Result<Reply, HidppError> {
        let function_byte = (function << 4) | SW_ID;
        flush(self.io)?;
        self.io.write(&[
            REPORT_ID_SHORT,
            self.index,
            feature_index,
            function_byte,
            payload[0],
            payload[1],
            payload[2],
        ])?;

        let deadline = self.io.now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(self.io.now());
            let timeout_ms = remaining.as_millis().min(i32::MAX as u128) as i32;
            let mut report = Report {
                buffer: [0; REPORT_BUFFER_LEN],
                length: 0,
            };
            report.length = self.io.read_timeout(&mut report.buffer, timeout_ms)?;
            if let Some(reply) = match_report(report, self.index, feature_index, function_byte) {
                return Ok(reply);
            }
            if self.io.now() >= deadline {
                return Ok(Reply::Silence);
            }
        }
    }

    fn lookup_feature(
        &self,
        feature_id: u16,
        timeout: Duration,
    ) -> Result<FeatureLookup, HidppError> {
        let [feature_high, feature_low] = feature_id.to_be_bytes();
        let reply = self.request(FEATURE_ROOT, 0, [feature_high, feature_low, 0], timeout)?;
        Ok(match reply {
            Reply::Response(report) => match parse_feature_index(report.bytes())? {
                Some(feature_index) => FeatureLookup::Index(feature_index),
                None => FeatureLookup::Unsupported,
            },
            Reply::DeviceError => FeatureLookup::Unsupported,
            Reply::Offline => FeatureLookup::Offline,
            Reply::Silence => FeatureLookup::Silent,
        })
    }

    fn find_battery_feature(&self, deadline: Instant) -> Result<Discovery, HidppError> {
        let unified =
            self.lookup_feature(FEATURE_UNIFIED_BATTERY, request_timeout(self.io, deadline))?;
        match unified {
            FeatureLookup::Index(feature_index) => {
                return Ok(Discovery::Found(BatteryFeature::Unified(feature_index)))
            }
            FeatureLookup::Offline => return Ok(Discovery::Offline),
            // Every HID++ 2.0 device answers root feature lookups, so silence means
            // this index is absent and the legacy lookup would only wait again.
            FeatureLookup::Silent => return Ok(Discovery::NotFound),
            FeatureLookup::Unsupported => {}
        }

        let legacy =
            self.lookup_feature(FEATURE_BATTERY_STATUS, request_timeout(self.io, deadline))?;
        Ok(match legacy {
            FeatureLookup::Index(feature_index) => {
                Discovery::Found(BatteryFeature::Legacy(feature_index))
            }
            FeatureLookup::Offline => Discovery::Offline,
            FeatureLookup::Unsupported | FeatureLookup::Silent => Discovery::NotFound,
        })
    }

    fn query_battery(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError> {
        // Unified Battery reads its status with function 1, Battery Status with
        // function 0.
        let (feature_index, function, charging_offset) = match feature {
            BatteryFeature::Unified(index) => (index, 0x01, 7),
            BatteryFeature::Legacy(index) => (index, 0x00, 6),
        };
        match self.request(feature_index, function, [0; 3], RESPONSE_TIMEOUT)? {
            Reply::Response(report) => Ok(Some(parse_battery(report.bytes(), charging_offset)?)),
            Reply::DeviceError | Reply::Offline | Reply::Silence => Ok(None),
        }
    }

    /// Read the discovered feature. A unified read that fails to produce a
    /// status falls back to the legacy feature, unless the transport failed.
    fn read_feature(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError> {
        if matches!(feature, BatteryFeature::Legacy(_)) {
            return self.query_battery(feature);
        }

        let unified_result = self.query_battery(feature);
        let query_legacy = || match self.lookup_feature(FEATURE_BATTERY_STATUS, RESPONSE_TIMEOUT)? {
            FeatureLookup::Index(index) => self.query_battery(BatteryFeature::Legacy(index)),
            FeatureLookup::Unsupported | FeatureLookup::Offline | FeatureLookup::Silent => Ok(None),
        };

        match unified_result {
            Ok(Some(status)) => Ok(Some(status)),
            Ok(None) => query_legacy(),
            Err(error @ HidppError::ShortResponse { .. }) => match query_legacy() {
                Ok(Some(status)) => Ok(Some(status)),
                Ok(None) | Err(HidppError::ShortResponse { .. }) => Err(error),
                Err(legacy_error @ HidppError::Hid(_)) => Err(legacy_error),
            },
            Err(error) => Err(error),
        }
    }
}

fn candidate_interfaces(api: &HidApi) -> Vec<&hidapi::DeviceInfo> {
    let mut candidates = api
        .device_list()
        .filter(|info| info.vendor_id() == LOGITECH_VID && info.usage_page() == HIDPP_USAGE_PAGE)
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        left.product_id()
            .cmp(&right.product_id())
            .then_with(|| left.interface_number().cmp(&right.interface_number()))
            .then_with(|| left.path().to_bytes().cmp(right.path().to_bytes()))
    });
    candidates
}

/// Read the battery status through the first candidate HID interface and
/// device index that expose a battery feature.
pub fn read_battery(api: &HidApi) -> Result<Reading, HidppError> {
    let interfaces = candidate_interfaces(api)
        .into_iter()
        .map(|info| move || api.open_path(info.path()).map_err(HidppError::Hid));
    read_battery_from(interfaces, Instant::now)
}

/// A HID interface, opened on first use and reused across device indices.
enum Slot<Open, Io> {
    Unopened(Open),
    Open(Io),
    Failed,
}

impl<Open: FnOnce() -> Result<Io, HidppError>, Io> Slot<Open, Io> {
    /// The open HID interface, or `None` if it failed to open. Only the first
    /// failed open reports its error; later probes skip the slot.
    fn open(&mut self) -> Result<Option<&Io>, HidppError> {
        *self = match std::mem::replace(self, Self::Failed) {
            Self::Unopened(open) => Self::Open(open()?),
            slot => slot,
        };
        Ok(match self {
            Self::Open(io) => Some(io),
            Self::Unopened(_) | Self::Failed => None,
        })
    }
}

/// Probe index-major, so the likely device indices on every HID interface come
/// before the unlikely ones on the first, then read the first battery feature
/// found. `interfaces` open the candidate HID interfaces in probe order.
fn read_battery_from<Io: RequestIo>(
    interfaces: impl IntoIterator<Item = impl FnOnce() -> Result<Io, HidppError>>,
    now: impl Fn() -> Instant,
) -> Result<Reading, HidppError> {
    let mut slots = interfaces
        .into_iter()
        .map(Slot::Unopened)
        .collect::<Vec<_>>();
    let deadline = now() + DISCOVERY_BUDGET;
    let mut offline = false;
    let mut last_error = None;
    'probe: for index in DEVICE_INDEX_CANDIDATES {
        for slot in &mut slots {
            if now() >= deadline {
                break 'probe;
            }
            let io = match slot.open() {
                Ok(Some(io)) => io,
                Ok(None) => continue,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };
            let device = Device { io, index };
            match device.find_battery_feature(deadline) {
                // The first battery feature found ends discovery, even if
                // reading it fails.
                Ok(Discovery::Found(feature)) => {
                    return Ok(match device.read_feature(feature)? {
                        Some(status) => Reading::Status(status),
                        None => Reading::NoStatus,
                    });
                }
                Ok(Discovery::Offline) => offline = true,
                Ok(Discovery::NotFound) => {}
                Err(error) => last_error = Some(error),
            }
        }
    }
    // The receiver's own report explains the failure better than an error from
    // another HID interface.
    if offline {
        return Ok(Reading::Offline);
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(Reading::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::iter;
    use std::rc::Rc;

    use super::*;

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
    struct ScriptedIo {
        // Sharing this cell keeps every scripted HID interface on one timeline.
        clock: Rc<Cell<Instant>>,
        steps: RefCell<VecDeque<ScriptStep>>,
    }

    impl ScriptedIo {
        fn next_step(&self, call: &str) -> ScriptStep {
            self.steps
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| panic!("script exhausted during {call}"))
        }
    }

    impl Drop for ScriptedIo {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                let steps = self.steps.get_mut();
                assert!(steps.is_empty(), "unplayed script steps: {steps:?}");
            }
        }
    }

    impl RequestIo for ScriptedIo {
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
        let clock = Rc::new(Cell::new(start));
        let openers = interfaces.into_iter().map(|interface| {
            let clock = Rc::clone(&clock);
            move || {
                Ok(ScriptedIo {
                    clock,
                    steps: RefCell::new(interface?.into()),
                })
            }
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
            let (result, elapsed) = read_one(DEVICE_INDEX_CANDIDATES.map(|device_index| {
                exchange(
                    &unified_lookup(device_index),
                    [reply(&receiver_offline(device_index)).after(reply_delay)],
                )
            }));

            assert!(matches!(result, Ok(Reading::Offline)));
            assert_eq!(elapsed, reply_delay * 7);
        }

        #[test]
        fn continues_from_an_offline_receiver_to_direct_usb() {
            let (result, _) = read_one([
                offline_probe(1),
                exchange(
                    &unified_lookup(0xFF),
                    [reply(&[REPORT_ID_SHORT, 0xFF, 0, 0x0F, 9, 0, 0])],
                ),
                exchange(
                    &[REPORT_ID_SHORT, 0xFF, 9, 0x1F, 0, 0, 0],
                    [reply(&[REPORT_ID_LONG, 0xFF, 9, 0x1F, 53, 0, 0, 1])],
                ),
            ]);

            assert_eq!(result.unwrap(), status(53, true));
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
            let (result, _) =
                read_scripted([unopenable(), script([1, 0xFF, 2, 3].map(silent_probe))]);
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
                        ScriptStep::read(1_500, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0])
                            .after(ms(500)),
                        ScriptStep::read(1_000, &[REPORT_ID_SHORT, 1, 0, 0x0E, 5, 0, 0])
                            .after(ms(750)),
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
}
