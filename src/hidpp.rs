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
    ShortWrite {
        expected: usize,
        actual: usize,
    },
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
            Self::ShortWrite { expected, actual } => write!(
                formatter,
                "short HID++ write: expected {expected} bytes, wrote {actual}"
            ),
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

/// What a single HID++ request produced.
enum Reply {
    Response {
        buffer: [u8; REPORT_BUFFER_LEN],
        length: usize,
    },
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

/// What probing one device index, or every interface and device index, found.
#[derive(Debug)]
pub enum Discovery<D> {
    Found(D),
    /// A receiver answered, but reported no connected mouse.
    Offline,
    NotFound,
}

impl<D> Discovery<D> {
    fn map<E>(self, found: impl FnOnce(D) -> E) -> Discovery<E> {
        match self {
            Self::Found(value) => Discovery::Found(found(value)),
            Self::Offline => Discovery::Offline,
            Self::NotFound => Discovery::NotFound,
        }
    }
}

/// An open HID++ interface with the device index and battery feature that
/// discovery selected.
pub struct HidppTransport {
    device: HidDevice,
    device_index: u8,
    battery_feature: BatteryFeature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatteryFeature {
    Unified(u8),
    Legacy(u8),
}

/// Report I/O and the monotonic timeline used by a request.
trait RequestIo {
    fn write(&self, report: &[u8]) -> Result<usize, HidError>;
    fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, HidError>;
    fn now(&self) -> Instant;
}

impl RequestIo for HidDevice {
    fn write(&self, report: &[u8]) -> Result<usize, HidError> {
        HidDevice::write(self, report)
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
fn flush(device: &impl RequestIo) -> Result<(), HidppError> {
    let deadline = device.now() + FLUSH_WINDOW;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    while device.now() < deadline {
        if device.read_timeout(&mut buffer, 0)? == 0 {
            break;
        }
    }
    Ok(())
}

/// Remaining time for one request, capped at the per-request timeout.
fn request_timeout(device: &impl RequestIo, deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(device.now())
        .min(RESPONSE_TIMEOUT)
}

fn send_short(device: &impl RequestIo, report: [u8; 7]) -> Result<(), HidppError> {
    let actual = device.write(&report)?;
    if actual != report.len() {
        return Err(HidppError::ShortWrite {
            expected: report.len(),
            actual,
        });
    }
    Ok(())
}

/// How a received report relates to the request awaiting a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportMatch {
    Reply,
    /// The receiver has no connected device at the requested index.
    Offline,
    /// The device rejected the request.
    DeviceError,
    Unrelated,
}

/// A HID++ 2.0 reply echoes the request's function and software ID, so matching
/// that byte too rejects stale reports and replies meant for another reader.
/// An error arrives as an ordinary short or long report whose third byte is
/// the error marker, followed by the request's feature index, function byte,
/// and the error code.
fn match_report(
    report: &[u8],
    device_index: u8,
    feature_index: u8,
    function_byte: u8,
) -> ReportMatch {
    let header_matches = matches!(
        report.first(),
        Some(&REPORT_ID_SHORT) | Some(&REPORT_ID_LONG)
    ) && report.get(1) == Some(&device_index);
    if !header_matches {
        return ReportMatch::Unrelated;
    }
    match report[2..] {
        [feature, function, ..] if feature == feature_index && function == function_byte => {
            ReportMatch::Reply
        }
        [marker @ (ERROR_MARKER_HIDPP10 | ERROR_MARKER_HIDPP20), feature, function, code, ..]
            if feature == feature_index && function == function_byte =>
        {
            if marker == ERROR_MARKER_HIDPP10 && code == ERROR_CODE_UNKNOWN_DEVICE {
                ReportMatch::Offline
            } else {
                ReportMatch::DeviceError
            }
        }
        _ => ReportMatch::Unrelated,
    }
}

fn read_response(
    device: &impl RequestIo,
    device_index: u8,
    feature_index: u8,
    function_byte: u8,
    timeout: Duration,
) -> Result<Reply, HidppError> {
    let deadline = device.now() + timeout;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(device.now());
        let timeout_ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        let length = device.read_timeout(&mut buffer, timeout_ms)?;
        if length > 0 {
            match match_report(
                &buffer[..length],
                device_index,
                feature_index,
                function_byte,
            ) {
                ReportMatch::Reply => return Ok(Reply::Response { buffer, length }),
                ReportMatch::Offline => return Ok(Reply::Offline),
                ReportMatch::DeviceError => return Ok(Reply::DeviceError),
                ReportMatch::Unrelated => {}
            }
        }
        if device.now() >= deadline {
            return Ok(Reply::Silence);
        }
    }
}

/// Send one short request and wait for its reply.
fn request(
    device: &impl RequestIo,
    device_index: u8,
    feature_index: u8,
    function: u8,
    payload: [u8; 3],
    timeout: Duration,
) -> Result<Reply, HidppError> {
    let function_byte = (function << 4) | SW_ID;
    flush(device)?;
    send_short(
        device,
        [
            REPORT_ID_SHORT,
            device_index,
            feature_index,
            function_byte,
            payload[0],
            payload[1],
            payload[2],
        ],
    )?;
    read_response(device, device_index, feature_index, function_byte, timeout)
}

fn lookup_feature(
    device: &impl RequestIo,
    device_index: u8,
    feature_id: u16,
    timeout: Duration,
) -> Result<FeatureLookup, HidppError> {
    let [feature_high, feature_low] = feature_id.to_be_bytes();
    let reply = request(
        device,
        device_index,
        FEATURE_ROOT,
        0,
        [feature_high, feature_low, 0],
        timeout,
    )?;
    Ok(match reply {
        Reply::Response { buffer, length } => match parse_feature_index(&buffer[..length])? {
            Some(feature_index) => FeatureLookup::Index(feature_index),
            None => FeatureLookup::Unsupported,
        },
        Reply::DeviceError => FeatureLookup::Unsupported,
        Reply::Offline => FeatureLookup::Offline,
        Reply::Silence => FeatureLookup::Silent,
    })
}

fn query_feature(
    device: &impl RequestIo,
    device_index: u8,
    feature: BatteryFeature,
) -> Result<Option<BatteryStatus>, HidppError> {
    // Unified Battery reads its status with function 1, Battery Status with
    // function 0.
    let (feature_index, function, charging_offset) = match feature {
        BatteryFeature::Unified(index) => (index, 0x01, 7),
        BatteryFeature::Legacy(index) => (index, 0x00, 6),
    };
    let reply = request(
        device,
        device_index,
        feature_index,
        function,
        [0; 3],
        RESPONSE_TIMEOUT,
    )?;
    match reply {
        Reply::Response { buffer, length } => {
            Ok(Some(parse_battery(&buffer[..length], charging_offset)?))
        }
        Reply::DeviceError | Reply::Offline | Reply::Silence => Ok(None),
    }
}

/// Read the discovered feature. A unified read that fails to produce a status
/// falls back to the legacy feature, unless the transport failed.
fn read_battery_from(
    device: &impl RequestIo,
    device_index: u8,
    feature: BatteryFeature,
) -> Result<Option<BatteryStatus>, HidppError> {
    if matches!(feature, BatteryFeature::Legacy(_)) {
        return query_feature(device, device_index, feature);
    }

    let unified_result = query_feature(device, device_index, feature);
    let query_legacy = || match lookup_feature(
        device,
        device_index,
        FEATURE_BATTERY_STATUS,
        RESPONSE_TIMEOUT,
    )? {
        FeatureLookup::Index(index) => {
            query_feature(device, device_index, BatteryFeature::Legacy(index))
        }
        FeatureLookup::Unsupported | FeatureLookup::Offline | FeatureLookup::Silent => Ok(None),
    };

    match unified_result {
        Ok(Some(status)) => Ok(Some(status)),
        Ok(None) => query_legacy(),
        Err(error @ HidppError::ShortResponse { .. }) => match query_legacy() {
            Ok(Some(status)) => Ok(Some(status)),
            Ok(None) | Err(HidppError::ShortResponse { .. }) => Err(error),
            Err(legacy_error @ (HidppError::Hid(_) | HidppError::ShortWrite { .. })) => {
                Err(legacy_error)
            }
        },
        Err(error) => Err(error),
    }
}

pub fn read_battery(transport: &HidppTransport) -> Result<Option<BatteryStatus>, HidppError> {
    read_battery_from(
        &transport.device,
        transport.device_index,
        transport.battery_feature,
    )
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

fn find_battery_feature(
    device: &impl RequestIo,
    device_index: u8,
    deadline: Instant,
) -> Result<Discovery<BatteryFeature>, HidppError> {
    let unified = lookup_feature(
        device,
        device_index,
        FEATURE_UNIFIED_BATTERY,
        request_timeout(device, deadline),
    )?;
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

    let legacy = lookup_feature(
        device,
        device_index,
        FEATURE_BATTERY_STATUS,
        request_timeout(device, deadline),
    )?;
    Ok(match legacy {
        FeatureLookup::Index(feature_index) => {
            Discovery::Found(BatteryFeature::Legacy(feature_index))
        }
        FeatureLookup::Offline => Discovery::Offline,
        FeatureLookup::Unsupported | FeatureLookup::Silent => Discovery::NotFound,
    })
}

/// A probed interface, opened on first use and reused across device indices.
enum ProbeSlot {
    Unopened,
    Open(HidDevice),
    Failed,
}

fn open_probe_device(api: &HidApi, info: &hidapi::DeviceInfo) -> Result<HidDevice, HidppError> {
    let device = api.open_path(info.path())?;
    device.set_blocking_mode(false)?;
    Ok(device)
}

/// The interface in this slot, opened on first use. Only the first failed open
/// reports its error; later probes skip the slot.
fn probe_device<'slot>(
    api: &HidApi,
    info: &hidapi::DeviceInfo,
    slot: &'slot mut ProbeSlot,
) -> Result<Option<&'slot HidDevice>, HidppError> {
    if matches!(slot, ProbeSlot::Unopened) {
        match open_probe_device(api, info) {
            Ok(device) => *slot = ProbeSlot::Open(device),
            Err(error) => {
                *slot = ProbeSlot::Failed;
                return Err(error);
            }
        }
    }
    Ok(match slot {
        ProbeSlot::Open(device) => Some(device),
        ProbeSlot::Unopened | ProbeSlot::Failed => None,
    })
}

/// Probe index-major: the likely indices on every interface come before the
/// unlikely ones on the first interface.
fn discover<D>(
    interface_count: usize,
    deadline: Instant,
    now: impl Fn() -> Instant,
    mut probe: impl FnMut(usize, u8) -> Result<Discovery<D>, HidppError>,
) -> Result<Discovery<D>, HidppError> {
    let mut offline = false;
    let mut last_error = None;
    'probe: for device_index in DEVICE_INDEX_CANDIDATES {
        for interface in 0..interface_count {
            if now() >= deadline {
                break 'probe;
            }
            match probe(interface, device_index) {
                Ok(Discovery::Found(found)) => return Ok(Discovery::Found(found)),
                Ok(Discovery::Offline) => offline = true,
                Ok(Discovery::NotFound) => {}
                Err(error) => last_error = Some(error),
            }
        }
    }
    // The receiver's own report explains the failure better than an error from
    // another interface.
    if offline {
        return Ok(Discovery::Offline);
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(Discovery::NotFound),
    }
}

pub fn open_first_working_transport(api: &HidApi) -> Result<Discovery<HidppTransport>, HidppError> {
    let candidates = candidate_interfaces(api);
    let mut slots = candidates
        .iter()
        .map(|_| ProbeSlot::Unopened)
        .collect::<Vec<_>>();
    let deadline = Instant::now() + DISCOVERY_BUDGET;

    discover(
        candidates.len(),
        deadline,
        Instant::now,
        |interface, device_index| {
            let slot = &mut slots[interface];
            let Some(device) = probe_device(api, candidates[interface], slot)? else {
                return Ok(Discovery::NotFound);
            };
            let outcome = find_battery_feature(device, device_index, deadline)?;
            Ok(outcome.map(|battery_feature| {
                let ProbeSlot::Open(device) = std::mem::replace(slot, ProbeSlot::Failed) else {
                    unreachable!("a found battery feature comes from an open interface");
                };
                HidppTransport {
                    device,
                    device_index,
                    battery_feature,
                }
            }))
        },
    )
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use super::*;

    const UNIFIED_LOOKUP: [u8; 7] = unified_lookup(1);
    const LEGACY_LOOKUP: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x00, 0];
    const UNIFIED_QUERY: [u8; 7] = [REPORT_ID_SHORT, 1, 5, 0x1F, 0, 0, 0];
    const LEGACY_QUERY: [u8; 7] = [REPORT_ID_SHORT, 1, 6, 0x0F, 0, 0, 0];
    const UNIFIED_FEATURE: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 5, 0, 0];
    const LEGACY_FEATURE: [u8; 7] = [REPORT_ID_SHORT, 1, 0, 0x0F, 6, 0, 0];
    const LEGACY_STATUS: [u8; 7] = [REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0, 0x81];
    const LEGACY_LEVEL: BatteryStatus = BatteryStatus {
        level: 42,
        charging: true,
    };
    /// A unified status reply that lacks the charging byte.
    const INCOMPLETE_UNIFIED: [u8; 7] = [REPORT_ID_SHORT, 1, 5, 0x1F, 99, 0, 0];
    // Captured from a LIGHTSPEED receiver while its paired mouse was off.
    const RECEIVER_OFFLINE: [u8; 7] = [REPORT_ID_SHORT, 1, 0x8F, 0, 0x0F, 0x08, 0];

    const fn unified_lookup(device_index: u8) -> [u8; 7] {
        [REPORT_ID_SHORT, device_index, 0, 0x0F, 0x10, 0x04, 0]
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
            result: Result<usize, HidError>,
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
                result: Ok(report.len()),
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

    /// Plays back an exact sequence of I/O calls on a virtual clock. Any call
    /// the script does not expect fails the test, and so does any step still
    /// unplayed when the script is dropped.
    struct ScriptedIo {
        // Sharing this cell keeps multiple scripted interfaces on one timeline.
        clock: Rc<Cell<Instant>>,
        start: Instant,
        steps: RefCell<VecDeque<ScriptStep>>,
    }

    impl ScriptedIo {
        fn new<S: IntoIterator<Item = ScriptStep>>(parts: impl IntoIterator<Item = S>) -> Self {
            let start = Instant::now();
            Self {
                clock: Rc::new(Cell::new(start)),
                start,
                steps: RefCell::new(parts.into_iter().flatten().collect()),
            }
        }

        fn elapsed(&self) -> Duration {
            self.now() - self.start
        }

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
        fn write(&self, report: &[u8]) -> Result<usize, HidError> {
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

    /// Discover the battery feature on `device_index`, then read it.
    fn probe_and_read(
        io: &ScriptedIo,
        device_index: u8,
    ) -> Result<Option<BatteryStatus>, HidppError> {
        let Ok(Discovery::Found(battery_feature)) =
            find_battery_feature(io, device_index, io.now() + DISCOVERY_BUDGET)
        else {
            panic!("a battery feature should be found on device index {device_index}");
        };
        read_battery_from(io, device_index, battery_feature)
    }

    /// Discover on a single interface.
    fn discover_single(
        io: &ScriptedIo,
        deadline: Instant,
    ) -> Result<Discovery<BatteryFeature>, HidppError> {
        discover(
            1,
            deadline,
            || io.now(),
            |_, device_index| find_battery_feature(io, device_index, deadline),
        )
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
        let cases: [(&[u8], ReportMatch); 14] = [
            (&[REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0], ReportMatch::Reply),
            (
                &[REPORT_ID_LONG, 1, 5, 0x1F, 78, 0, 0, 1],
                ReportMatch::Reply,
            ),
            (&[0x12, 1, 5, 0x1F, 78, 0, 0], ReportMatch::Unrelated),
            (
                &[REPORT_ID_SHORT, 2, 5, 0x1F, 78, 0, 0],
                ReportMatch::Unrelated,
            ),
            (
                &[REPORT_ID_SHORT, 1, 6, 0x1F, 78, 0, 0],
                ReportMatch::Unrelated,
            ),
            (
                &[REPORT_ID_SHORT, 1, 5, 0x0F, 78, 0, 0],
                ReportMatch::Unrelated,
            ),
            (
                &[REPORT_ID_SHORT, 1, 5, 0x1E, 78, 0, 0],
                ReportMatch::Unrelated,
            ),
            (&[REPORT_ID_SHORT, 1], ReportMatch::Unrelated),
            (
                &[REPORT_ID_SHORT, 1, 0x8F, 5, 0x1F, 0x08, 0],
                ReportMatch::Offline,
            ),
            (
                &[REPORT_ID_SHORT, 1, 0x8F, 5, 0x1F, 0x02, 0],
                ReportMatch::DeviceError,
            ),
            // Only the receiver's HID++ 1.0 error means no connected device.
            (
                &[REPORT_ID_LONG, 1, 0xFF, 5, 0x1F, 0x08, 0],
                ReportMatch::DeviceError,
            ),
            // Errors must echo the request too.
            (
                &[REPORT_ID_SHORT, 1, 0x8F, 6, 0x1F, 0x08, 0],
                ReportMatch::Unrelated,
            ),
            (
                &[REPORT_ID_SHORT, 1, 0xFF, 5, 0x0F, 0x02, 0],
                ReportMatch::Unrelated,
            ),
            (
                &[REPORT_ID_SHORT, 1, 0x8E, 5, 0x1F, 0x02, 0],
                ReportMatch::Unrelated,
            ),
        ];
        for (report, expected) in cases {
            assert_eq!(
                match_report(report, 1, 5, 0x1F),
                expected,
                "report {report:02X?}"
            );
        }
    }

    mod discovery {
        use super::*;

        #[test]
        fn skips_unrelated_reports_and_errors_until_the_matching_reply() {
            let io = ScriptedIo::new([
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

            assert_eq!(
                probe_and_read(&io, 1).unwrap(),
                Some(BatteryStatus {
                    level: 78,
                    charging: true,
                })
            );
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
                let io = ScriptedIo::new([
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

                assert_eq!(
                    probe_and_read(&io, device_index).unwrap(),
                    Some(BatteryStatus { level, charging })
                );
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
                let io = ScriptedIo::new([
                    exchange(&UNIFIED_LOOKUP, [reply(&unified_reply)]),
                    exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE)]),
                    exchange(&LEGACY_QUERY, [reply(&LEGACY_STATUS)]),
                ]);

                assert_eq!(probe_and_read(&io, 1).unwrap(), Some(LEGACY_LEVEL));
            }
        }

        #[test]
        fn receiver_offline_ends_the_probe_without_waiting_or_legacy_lookup() {
            let reply_delay = ms(17);
            let io = ScriptedIo::new([exchange(
                &UNIFIED_LOOKUP,
                [reply(&RECEIVER_OFFLINE).after(reply_delay)],
            )]);

            let result = find_battery_feature(&io, 1, io.now() + DISCOVERY_BUDGET);

            assert!(matches!(result, Ok(Discovery::Offline)));
            assert_eq!(io.elapsed(), reply_delay);
        }

        #[test]
        fn silence_after_an_unmatched_reply_skips_legacy_lookup() {
            let io = ScriptedIo::new([exchange(
                &UNIFIED_LOOKUP,
                [reply(&[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]), no_reply()],
            )]);

            let result = find_battery_feature(&io, 1, io.now() + DISCOVERY_BUDGET);

            assert!(matches!(result, Ok(Discovery::NotFound)));
            assert_eq!(io.elapsed(), RESPONSE_TIMEOUT);
        }

        #[test]
        fn continues_from_an_offline_receiver_to_direct_usb() {
            let io = ScriptedIo::new([
                exchange(&UNIFIED_LOOKUP, [reply(&RECEIVER_OFFLINE)]),
                exchange(
                    &unified_lookup(0xFF),
                    [reply(&[REPORT_ID_SHORT, 0xFF, 0, 0x0F, 9, 0, 0])],
                ),
                exchange(
                    &[REPORT_ID_SHORT, 0xFF, 9, 0x1F, 0, 0, 0],
                    [reply(&[REPORT_ID_LONG, 0xFF, 9, 0x1F, 53, 0, 0, 1])],
                ),
            ]);
            let deadline = io.now() + DISCOVERY_BUDGET;

            let found = discover(
                1,
                deadline,
                || io.now(),
                |_, device_index| {
                    Ok(find_battery_feature(&io, device_index, deadline)?
                        .map(|battery_feature| (device_index, battery_feature)))
                },
            );

            let Ok(Discovery::Found((device_index, battery_feature))) = found else {
                panic!("discovery should continue to the direct USB candidate");
            };
            assert_eq!(
                read_battery_from(&io, device_index, battery_feature).unwrap(),
                Some(BatteryStatus {
                    level: 53,
                    charging: true,
                })
            );
        }

        #[test]
        fn checks_likely_indices_on_every_interface_first() {
            let first = ScriptedIo::new([
                exchange(&UNIFIED_LOOKUP, [reply(&RECEIVER_OFFLINE)]),
                exchange(
                    &unified_lookup(0xFF),
                    [reply(&[REPORT_ID_SHORT, 0xFF, 0x8F, 0, 0x0F, 0x08, 0])],
                ),
            ]);
            let mut second = ScriptedIo::new([
                exchange(&UNIFIED_LOOKUP, [reply(&RECEIVER_OFFLINE)]),
                exchange(
                    &unified_lookup(0xFF),
                    [reply(&[REPORT_ID_SHORT, 0xFF, 0, 0x0F, 9, 0, 0])],
                ),
            ]);
            second.clock = Rc::clone(&first.clock);
            let interfaces = [first, second];
            let deadline = interfaces[0].now() + DISCOVERY_BUDGET;

            let result = discover(
                interfaces.len(),
                deadline,
                || interfaces[0].now(),
                |interface, device_index| {
                    find_battery_feature(&interfaces[interface], device_index, deadline)
                },
            );

            assert!(matches!(
                result,
                Ok(Discovery::Found(BatteryFeature::Unified(9)))
            ));
        }

        #[test]
        fn offline_outranks_later_errors_while_preserving_candidate_order() {
            for offline_first in [true, false] {
                let io = ScriptedIo::new([1, 0xFF, 2, 3, 4, 5, 6].map(|device_index| {
                    let truncated = [REPORT_ID_SHORT, device_index, 0, 0x0F];
                    let answer = if offline_first && device_index == 1 {
                        reply(&RECEIVER_OFFLINE)
                    } else {
                        reply(&truncated)
                    };
                    exchange(&unified_lookup(device_index), [answer])
                }));

                let result = discover_single(&io, io.now() + DISCOVERY_BUDGET);

                if offline_first {
                    assert!(matches!(result, Ok(Discovery::Offline)));
                } else {
                    assert_short_response(result, 5, 4);
                }
            }
        }
    }

    mod request_timing {
        use super::*;

        fn silent_probe(device_index: u8) -> Vec<ScriptStep> {
            exchange(&unified_lookup(device_index), [no_reply()])
        }

        #[test]
        fn continuous_queued_reports_stop_flushing_at_the_window_boundary() {
            for last_read_ms in [1, 2] {
                let stale = [REPORT_ID_SHORT, 1, 5, 0x1F, 8, 0, 0, 0];
                let io = ScriptedIo::new([[
                    ScriptStep::flush(&stale).after(ms(100)),
                    ScriptStep::flush(&stale).after(ms(99)),
                    ScriptStep::flush(&stale).after(ms(last_read_ms)),
                    ScriptStep::write(&UNIFIED_QUERY),
                    reply(&[REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0, 1]),
                ]]);

                assert_eq!(
                    read_battery_from(&io, 1, BatteryFeature::Unified(5)).unwrap(),
                    Some(BatteryStatus {
                        level: 78,
                        charging: true,
                    })
                );
                assert_eq!(io.elapsed(), ms(199 + last_read_ms));
            }
        }

        #[test]
        fn unrelated_reports_consume_the_original_response_wait() {
            let io = ScriptedIo::new([exchange(
                &UNIFIED_LOOKUP,
                [
                    ScriptStep::read(1_500, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]).after(ms(500)),
                    ScriptStep::read(1_000, &[REPORT_ID_SHORT, 1, 0, 0x0E, 5, 0, 0]).after(ms(750)),
                    ScriptStep::silence(250, ms(250)),
                ],
            )]);

            let result = lookup_feature(&io, 1, FEATURE_UNIFIED_BATTERY, RESPONSE_TIMEOUT);

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.elapsed(), RESPONSE_TIMEOUT);
        }

        #[test]
        fn fractional_milliseconds_are_truncated_without_ending_the_wait_early() {
            let unrelated = [REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0];
            let io = ScriptedIo::new([exchange(
                &UNIFIED_LOOKUP,
                [
                    ScriptStep::read(2, &unrelated).after(Duration::from_micros(1_750)),
                    ScriptStep::read(0, &unrelated).after(Duration::from_micros(500)),
                    ScriptStep::silence(0, Duration::from_micros(250)),
                ],
            )]);

            let result = lookup_feature(
                &io,
                1,
                FEATURE_UNIFIED_BATTERY,
                Duration::from_micros(2_500),
            );

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.elapsed(), Duration::from_micros(2_500));
        }

        #[test]
        fn zero_duration_ignores_an_unrelated_report_and_returns_silence() {
            let io = ScriptedIo::new([exchange(
                &UNIFIED_LOOKUP,
                [ScriptStep::read(0, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0])],
            )]);

            let result = lookup_feature(&io, 1, FEATURE_UNIFIED_BATTERY, Duration::ZERO);

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
        }

        #[test]
        fn discovery_overhead_preserves_the_wait_and_later_battery_request_durations() {
            let io = ScriptedIo::new([
                vec![
                    ScriptStep::flush(&[REPORT_ID_SHORT, 1, 0, 0x0F, 99, 0, 0]).after(ms(150)),
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&UNIFIED_LOOKUP).after(ms(100)),
                    ScriptStep::read(400, &UNIFIED_FEATURE).after(ms(400)),
                ],
                exchange(&UNIFIED_QUERY, [no_reply()]),
                exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE).after(ms(1_500))]),
                exchange(&LEGACY_QUERY, [reply(&LEGACY_STATUS).after(ms(1_500))]),
            ]);
            // Only 400 ms remain for discovery when this candidate starts.
            let deadline = io.now() + ms(400);

            let Ok(Discovery::Found(battery_feature)) = discover_single(&io, deadline) else {
                panic!("a matching reply at the response deadline should be accepted");
            };
            assert_eq!(battery_feature, BatteryFeature::Unified(5));
            assert_eq!(io.elapsed(), ms(650));

            assert_eq!(
                read_battery_from(&io, 1, battery_feature).unwrap(),
                Some(LEGACY_LEVEL)
            );
            assert_eq!(io.elapsed(), ms(5_150));
        }

        #[test]
        fn discovery_stops_before_the_next_candidate_after_six_seconds_of_silence() {
            let io = ScriptedIo::new([1, 0xFF, 2, 3].map(silent_probe));

            let result = discover_single(&io, io.now() + DISCOVERY_BUDGET);

            assert!(matches!(result, Ok(Discovery::NotFound)));
            assert_eq!(io.elapsed(), DISCOVERY_BUDGET);
        }

        #[test]
        fn a_probe_can_finish_its_legacy_lookup_after_the_discovery_budget_is_used() {
            let io = ScriptedIo::new([
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
            ]);

            let result = discover_single(&io, io.now() + DISCOVERY_BUDGET);

            assert!(matches!(
                result,
                Ok(Discovery::Found(BatteryFeature::Legacy(6)))
            ));
            assert_eq!(io.elapsed(), DISCOVERY_BUDGET);
        }
    }

    mod battery_recovery {
        use super::*;

        fn read_unified(io: &ScriptedIo) -> Result<Option<BatteryStatus>, HidppError> {
            read_battery_from(io, 1, BatteryFeature::Unified(5))
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
                let io = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED[..length])]),
                    legacy_fallback(reply(&LEGACY_STATUS)),
                ]);

                assert_eq!(read_unified(&io).unwrap(), Some(LEGACY_LEVEL));
            }
        }

        #[test]
        fn offline_device_error_and_silence_recover_with_a_legacy_status() {
            for unified_reply in [
                reply(&[REPORT_ID_SHORT, 1, ERROR_MARKER_HIDPP10, 5, 0x1F, 0x08, 0]),
                reply(&[REPORT_ID_LONG, 1, ERROR_MARKER_HIDPP20, 5, 0x1F, 0x02]),
                no_reply(),
            ] {
                let io = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [unified_reply]),
                    legacy_fallback(reply(&LEGACY_STATUS)),
                ]);

                assert_eq!(read_unified(&io).unwrap(), Some(LEGACY_LEVEL));
            }
        }

        #[test]
        fn missing_results_from_both_features_return_no_battery_status() {
            let io = ScriptedIo::new([
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

            assert_eq!(read_unified(&io).unwrap(), None);
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
                let io = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                    exchange(&LEGACY_LOOKUP, [lookup_reply]),
                ]);

                assert_short_response(read_unified(&io), 8, 7);
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
                let io = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                    legacy_fallback(status_reply),
                ]);

                assert_short_response(read_unified(&io), 8, 7);
            }
        }

        #[test]
        fn a_missing_unified_result_does_not_hide_a_legacy_short_response() {
            let io = ScriptedIo::new([
                exchange(&UNIFIED_QUERY, [no_reply()]),
                legacy_fallback(reply(&[REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0])),
            ]);

            assert_short_response(read_unified(&io), 7, 6);
        }

        #[derive(Debug, Clone, Copy)]
        enum IoFailure {
            FlushRead,
            Write,
            ShortWrite,
            ResponseRead,
        }

        impl IoFailure {
            const ALL: [Self; 4] = [
                Self::FlushRead,
                Self::Write,
                Self::ShortWrite,
                Self::ResponseRead,
            ];

            /// The script for `request` up to and including this failure.
            fn steps(self, request: &[u8]) -> Vec<ScriptStep> {
                let failed_read = |timeout_ms| ScriptStep::Read {
                    timeout_ms,
                    result: Err(HidError::InitializationError),
                    advance: Duration::ZERO,
                };
                let write = |result| ScriptStep::Write {
                    report: request.to_vec(),
                    result,
                    advance: Duration::ZERO,
                };
                match self {
                    Self::FlushRead => vec![failed_read(0)],
                    Self::Write => vec![
                        ScriptStep::flush(&[]),
                        write(Err(HidError::InitializationError)),
                    ],
                    Self::ShortWrite => vec![ScriptStep::flush(&[]), write(Ok(3))],
                    Self::ResponseRead => exchange(request, [failed_read(1_500)]),
                }
            }

            fn assert_error(self, result: Result<Option<BatteryStatus>, HidppError>) {
                match (self, result) {
                    (
                        Self::ShortWrite,
                        Err(HidppError::ShortWrite {
                            expected: 7,
                            actual: 3,
                        }),
                    )
                    | (
                        Self::FlushRead | Self::Write | Self::ResponseRead,
                        Err(HidppError::Hid(HidError::InitializationError)),
                    ) => {}
                    (failure, result) => panic!("{failure:?} produced {result:?}"),
                }
            }
        }

        #[test]
        fn unified_io_failures_stop_without_legacy_recovery() {
            for failure in IoFailure::ALL {
                let io = ScriptedIo::new([failure.steps(&UNIFIED_QUERY)]);

                failure.assert_error(read_unified(&io));
            }
        }

        #[test]
        fn legacy_io_failures_replace_the_unified_short_response() {
            for failure in IoFailure::ALL {
                let lookup_failure = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                    failure.steps(&LEGACY_LOOKUP),
                ]);
                failure.assert_error(read_unified(&lookup_failure));

                let query_failure = ScriptedIo::new([
                    exchange(&UNIFIED_QUERY, [reply(&INCOMPLETE_UNIFIED)]),
                    exchange(&LEGACY_LOOKUP, [reply(&LEGACY_FEATURE)]),
                    failure.steps(&LEGACY_QUERY),
                ]);
                failure.assert_error(read_unified(&query_failure));
            }
        }
    }
}
