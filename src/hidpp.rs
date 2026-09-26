use std::fmt;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice, HidError};

pub const LOGITECH_VID: u16 = 0x046D;
pub const HIDPP_USAGE_PAGE: u16 = 0xFF00;

pub const FEATURE_ROOT: u16 = 0x0000;
pub const FEATURE_BATTERY_STATUS: u16 = 0x1000;
pub const FEATURE_UNIFIED_BATTERY: u16 = 0x1004;

pub const REPORT_ID_SHORT: u8 = 0x10;
pub const REPORT_ID_LONG: u8 = 0x11;
/// Error markers in the third byte of a short or long report: receivers use
/// the HID++ 1.0 marker and HID++ 2.0 devices use their own.
const ERROR_MARKER_HIDPP10: u8 = 0x8F;
const ERROR_MARKER_HIDPP20: u8 = 0xFF;
/// HID++ 1.0 error code a receiver returns for a device index with no
/// connected device.
const ERROR_CODE_UNKNOWN_DEVICE: u8 = 0x08;
pub const DEFAULT_SW_ID: u8 = 0x0F;

/// Receiver-paired devices answer on 1..=6 and a directly connected device on
/// 0xFF, so probe both common slots before the rarely used ones.
pub const DEVICE_INDEX_CANDIDATES: &[u8] = &[1, 0xFF, 2, 3, 4, 5, 6];

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

/// A reply that matched the request but cannot be decoded. `match_report`
/// already checked the report ID, device index, feature index, and function
/// byte, and turned error reports into their own outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    ShortResponse { minimum: usize, actual: usize },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortResponse { minimum, actual } => write!(
                formatter,
                "short HID++ response: expected at least {minimum} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug)]
pub enum HidppError {
    Hid(HidError),
    Protocol(ProtocolError),
    ShortWrite { expected: usize, actual: usize },
}

impl fmt::Display for HidppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hid(error) => write!(formatter, "HID access failed: {error}"),
            Self::Protocol(error) => error.fmt(formatter),
            Self::ShortWrite { expected, actual } => write!(
                formatter,
                "short HID++ write: expected {expected} bytes, wrote {actual}"
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

impl From<ProtocolError> for HidppError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
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

/// What probing one device index on one interface found.
enum ProbeOutcome<D> {
    Found(D),
    Offline,
    Absent,
}

/// What probing every interface and device index found.
pub enum Discovery<D> {
    Found(D),
    /// A receiver answered, but reported no connected mouse.
    Offline,
    NotFound,
}

pub struct HidppTransport {
    device: HidDevice,
    pub device_index: u8,
    battery_feature: BatteryFeature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatteryFeature {
    Unified(u8),
    Legacy(u8),
}

trait BatteryDevice {
    fn selected_feature(&self) -> BatteryFeature;
    fn discover_feature(&self, feature_id: u16) -> Result<Option<u8>, HidppError>;
    fn query_feature(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError>;
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

/// A borrowed device with the feature selected by discovery.
struct BatterySession<'a, I> {
    io: &'a I,
    device_index: u8,
    battery_feature: BatteryFeature,
}

impl HidppTransport {
    fn session(&self) -> BatterySession<'_, HidDevice> {
        BatterySession {
            io: &self.device,
            device_index: self.device_index,
            battery_feature: self.battery_feature,
        }
    }
}

pub fn parse_feature_index(response: &[u8]) -> Result<Option<u8>, ProtocolError> {
    ensure_length(response, 5)?;
    let feature_index = response[4];
    Ok((feature_index != 0).then_some(feature_index))
}

pub fn parse_unified_battery(response: &[u8]) -> Result<BatteryStatus, ProtocolError> {
    parse_battery(response, 7)
}

pub fn parse_battery_status(response: &[u8]) -> Result<BatteryStatus, ProtocolError> {
    parse_battery(response, 6)
}

fn parse_battery(response: &[u8], charging_offset: usize) -> Result<BatteryStatus, ProtocolError> {
    ensure_length(response, charging_offset + 1)?;
    Ok(BatteryStatus {
        level: response[4],
        charging: response[charging_offset] & 0x01 != 0,
    })
}

fn ensure_length(response: &[u8], minimum_length: usize) -> Result<(), ProtocolError> {
    if response.len() < minimum_length {
        return Err(ProtocolError::ShortResponse {
            minimum: minimum_length,
            actual: response.len(),
        });
    }
    Ok(())
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

fn function_byte(function: u8, sw_id: u8) -> u8 {
    ((function & 0x0F) << 4) | (sw_id & 0x0F)
}

/// Remaining time for one request, capped at the per-request timeout.
fn request_timeout(device: &impl RequestIo, deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(device.now())
        .min(RESPONSE_TIMEOUT)
}

fn send_short(
    device: &impl RequestIo,
    device_index: u8,
    feature_index: u8,
    function_byte: u8,
    payload: [u8; 3],
) -> Result<(), HidppError> {
    let report = [
        REPORT_ID_SHORT,
        device_index,
        feature_index,
        function_byte,
        payload[0],
        payload[1],
        payload[2],
    ];
    let actual = device.write(&report)?;
    if actual != report.len() {
        return Err(HidppError::ShortWrite {
            expected: report.len(),
            actual,
        });
    }
    Ok(())
}

fn is_error_marker(byte: u8) -> bool {
    matches!(byte, ERROR_MARKER_HIDPP10 | ERROR_MARKER_HIDPP20)
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
        [marker, feature, function, code, ..]
            if is_error_marker(marker) && feature == feature_index && function == function_byte =>
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
    sw_id: u8,
    payload: [u8; 3],
    timeout: Duration,
) -> Result<Reply, HidppError> {
    let function_byte = function_byte(function, sw_id);
    flush(device)?;
    send_short(device, device_index, feature_index, function_byte, payload)?;
    read_response(device, device_index, feature_index, function_byte, timeout)
}

fn get_feature_index_on_device(
    device: &impl RequestIo,
    device_index: u8,
    sw_id: u8,
    feature_id: u16,
    timeout: Duration,
) -> Result<FeatureLookup, HidppError> {
    let reply = request(
        device,
        device_index,
        FEATURE_ROOT as u8,
        0,
        sw_id,
        [(feature_id >> 8) as u8, feature_id as u8, 0],
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

fn query_battery_on_device(
    device: &impl RequestIo,
    device_index: u8,
    sw_id: u8,
    feature_index: u8,
    function: u8,
    parse_response: fn(&[u8]) -> Result<BatteryStatus, ProtocolError>,
) -> Result<Option<BatteryStatus>, HidppError> {
    let reply = request(
        device,
        device_index,
        feature_index,
        function,
        sw_id,
        [0; 3],
        RESPONSE_TIMEOUT,
    )?;
    match reply {
        Reply::Response { buffer, length } => Ok(Some(parse_response(&buffer[..length])?)),
        Reply::DeviceError | Reply::Offline | Reply::Silence => Ok(None),
    }
}

pub fn get_feature_index(
    transport: &HidppTransport,
    feature_id: u16,
) -> Result<Option<u8>, HidppError> {
    transport.session().discover_feature(feature_id)
}

impl BatteryDevice for HidppTransport {
    fn selected_feature(&self) -> BatteryFeature {
        self.battery_feature
    }

    fn discover_feature(&self, feature_id: u16) -> Result<Option<u8>, HidppError> {
        get_feature_index(self, feature_id)
    }

    fn query_feature(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError> {
        self.session().query_feature(feature)
    }
}

impl<I: RequestIo> BatteryDevice for BatterySession<'_, I> {
    fn selected_feature(&self) -> BatteryFeature {
        self.battery_feature
    }

    fn discover_feature(&self, feature_id: u16) -> Result<Option<u8>, HidppError> {
        let lookup = get_feature_index_on_device(
            self.io,
            self.device_index,
            DEFAULT_SW_ID,
            feature_id,
            RESPONSE_TIMEOUT,
        )?;
        Ok(match lookup {
            FeatureLookup::Index(feature_index) => Some(feature_index),
            FeatureLookup::Unsupported | FeatureLookup::Offline | FeatureLookup::Silent => None,
        })
    }

    fn query_feature(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError> {
        let (feature_index, function, parser) = match feature {
            BatteryFeature::Unified(index) => (index, 0x01, parse_unified_battery as _),
            BatteryFeature::Legacy(index) => (index, 0x00, parse_battery_status as _),
        };
        query_battery_on_device(
            self.io,
            self.device_index,
            DEFAULT_SW_ID,
            feature_index,
            function,
            parser,
        )
    }
}

fn read_battery_from<T: BatteryDevice>(transport: &T) -> Result<Option<BatteryStatus>, HidppError> {
    let selected_feature = transport.selected_feature();
    if matches!(selected_feature, BatteryFeature::Legacy(_)) {
        return transport.query_feature(selected_feature);
    }

    let unified_result = transport.query_feature(selected_feature);
    let query_legacy = || {
        let Some(feature_index) = transport.discover_feature(FEATURE_BATTERY_STATUS)? else {
            return Ok(None);
        };
        transport.query_feature(BatteryFeature::Legacy(feature_index))
    };

    match unified_result {
        Ok(Some(status)) => Ok(Some(status)),
        Ok(None) => query_legacy(),
        Err(error @ HidppError::Protocol(_)) => match query_legacy() {
            Ok(Some(status)) => Ok(Some(status)),
            Ok(None) | Err(HidppError::Protocol(_)) => Err(error),
            Err(legacy_error @ (HidppError::Hid(_) | HidppError::ShortWrite { .. })) => {
                Err(legacy_error)
            }
        },
        Err(error) => Err(error),
    }
}

pub fn read_battery(transport: &HidppTransport) -> Result<Option<BatteryStatus>, HidppError> {
    read_battery_from(transport)
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
) -> Result<ProbeOutcome<BatteryFeature>, HidppError> {
    let unified = get_feature_index_on_device(
        device,
        device_index,
        DEFAULT_SW_ID,
        FEATURE_UNIFIED_BATTERY,
        request_timeout(device, deadline),
    )?;
    match unified {
        FeatureLookup::Index(feature_index) => {
            return Ok(ProbeOutcome::Found(BatteryFeature::Unified(feature_index)))
        }
        FeatureLookup::Offline => return Ok(ProbeOutcome::Offline),
        // Every HID++ 2.0 device answers root feature lookups, so silence means
        // this index is absent and the legacy lookup would only wait again.
        FeatureLookup::Silent => return Ok(ProbeOutcome::Absent),
        FeatureLookup::Unsupported => {}
    }

    let legacy = get_feature_index_on_device(
        device,
        device_index,
        DEFAULT_SW_ID,
        FEATURE_BATTERY_STATUS,
        request_timeout(device, deadline),
    )?;
    Ok(match legacy {
        FeatureLookup::Index(feature_index) => {
            ProbeOutcome::Found(BatteryFeature::Legacy(feature_index))
        }
        FeatureLookup::Offline => ProbeOutcome::Offline,
        FeatureLookup::Unsupported | FeatureLookup::Silent => ProbeOutcome::Absent,
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
    mut probe: impl FnMut(usize, u8) -> Result<ProbeOutcome<D>, HidppError>,
) -> Result<Discovery<D>, HidppError> {
    let mut offline = false;
    let mut last_error = None;
    'probe: for &device_index in DEVICE_INDEX_CANDIDATES {
        for interface in 0..interface_count {
            if now() >= deadline {
                break 'probe;
            }
            match probe(interface, device_index) {
                Ok(ProbeOutcome::Found(found)) => return Ok(Discovery::Found(found)),
                Ok(ProbeOutcome::Offline) => offline = true,
                Ok(ProbeOutcome::Absent) => {}
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
            let Some(device) = probe_device(api, candidates[interface], &mut slots[interface])?
            else {
                return Ok(ProbeOutcome::Absent);
            };
            let outcome = match find_battery_feature(device, device_index, deadline)? {
                ProbeOutcome::Found(battery_feature) => {
                    let ProbeSlot::Open(device) =
                        std::mem::replace(&mut slots[interface], ProbeSlot::Failed)
                    else {
                        return Ok(ProbeOutcome::Absent);
                    };
                    ProbeOutcome::Found(HidppTransport {
                        device,
                        device_index,
                        battery_feature,
                    })
                }
                ProbeOutcome::Offline => ProbeOutcome::Offline,
                ProbeOutcome::Absent => ProbeOutcome::Absent,
            };
            Ok(outcome)
        },
    )
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ReadPhase {
        Flush,
        Response,
    }

    /// An ordered script gates response reports behind their request's write.
    /// The phase is explicit because both phases can use a zero read timeout.
    #[derive(Debug)]
    enum ScriptStep {
        Read {
            phase: ReadPhase,
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
        fn flush(report: &[u8]) -> Self {
            Self::Read {
                phase: ReadPhase::Flush,
                timeout_ms: 0,
                result: Ok(report.to_vec()),
                advance: Duration::ZERO,
            }
        }

        fn write(report: &[u8]) -> Self {
            Self::Write {
                report: report.to_vec(),
                result: Ok(report.len()),
                advance: Duration::ZERO,
            }
        }

        fn response(timeout_ms: i32, report: &[u8]) -> Self {
            Self::Read {
                phase: ReadPhase::Response,
                timeout_ms,
                result: Ok(report.to_vec()),
                advance: Duration::ZERO,
            }
        }

        fn silence(timeout_ms: i32, advance: Duration) -> Self {
            Self::Read {
                phase: ReadPhase::Response,
                timeout_ms,
                result: Ok(Vec::new()),
                advance,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum IoCall {
        Read(i32),
        Write(Vec<u8>),
    }

    struct ScriptedIo {
        // Sharing this cell keeps multiple scripted interfaces on one timeline.
        clock: Rc<Cell<Instant>>,
        steps: RefCell<VecDeque<ScriptStep>>,
        calls: RefCell<Vec<IoCall>>,
    }

    impl ScriptedIo {
        fn new(steps: Vec<ScriptStep>) -> Self {
            let mut after_write = false;
            for step in &steps {
                match step {
                    ScriptStep::Write { .. } => after_write = true,
                    ScriptStep::Read {
                        phase: ReadPhase::Flush,
                        timeout_ms,
                        ..
                    } => {
                        assert_eq!(*timeout_ms, 0, "flush reads must be nonblocking");
                        after_write = false;
                    }
                    ScriptStep::Read {
                        phase: ReadPhase::Response,
                        result,
                        advance,
                        ..
                    } => {
                        assert!(
                            after_write,
                            "a scripted response requires a preceding write"
                        );
                        if matches!(result, Ok(report) if report.is_empty()) {
                            assert!(
                                !advance.is_zero(),
                                "scripted silence must advance virtual time"
                            );
                        }
                    }
                }
            }
            Self {
                clock: Rc::new(Cell::new(Instant::now())),
                steps: RefCell::new(steps.into()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn assert_finished(&self) {
            assert!(
                self.steps.borrow().is_empty(),
                "unconsumed script steps: {:?}",
                self.steps.borrow()
            );
        }

        fn writes(&self) -> Vec<Vec<u8>> {
            self.calls
                .borrow()
                .iter()
                .filter_map(|call| match call {
                    IoCall::Write(report) => Some(report.clone()),
                    IoCall::Read(_) => None,
                })
                .collect()
        }
    }

    impl RequestIo for ScriptedIo {
        fn write(&self, report: &[u8]) -> Result<usize, HidError> {
            self.calls.borrow_mut().push(IoCall::Write(report.to_vec()));
            let step = self
                .steps
                .borrow_mut()
                .pop_front()
                .expect("script exhausted during write");
            let ScriptStep::Write {
                report: expected,
                result,
                advance,
            } = step
            else {
                panic!("unexpected write; expected {step:?}");
            };
            assert_eq!(report, expected, "unexpected request bytes");
            self.clock.set(self.now() + advance);
            result
        }

        fn read_timeout(&self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, HidError> {
            self.calls.borrow_mut().push(IoCall::Read(timeout_ms));
            let step = self
                .steps
                .borrow_mut()
                .pop_front()
                .expect("script exhausted during read");
            let ScriptStep::Read {
                timeout_ms: expected,
                result,
                advance,
                ..
            } = step
            else {
                panic!("unexpected read; expected {step:?}");
            };
            assert_eq!(timeout_ms, expected, "unexpected read timeout");
            self.clock.set(self.now() + advance);
            let report = result?;
            assert!(
                report.len() <= buffer.len(),
                "scripted report exceeds buffer"
            );
            buffer[..report.len()].copy_from_slice(&report);
            Ok(report.len())
        }

        fn now(&self) -> Instant {
            self.clock.get()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TransportCall {
        Discover(u16),
        Query(BatteryFeature),
    }

    struct FakeTransport {
        selected_feature: BatteryFeature,
        feature_results: RefCell<VecDeque<Result<Option<u8>, HidppError>>>,
        query_results: RefCell<VecDeque<Result<Option<BatteryStatus>, HidppError>>>,
        calls: RefCell<Vec<TransportCall>>,
    }

    impl FakeTransport {
        fn new(
            selected_feature: BatteryFeature,
            feature_results: Vec<Result<Option<u8>, HidppError>>,
            query_results: Vec<Result<Option<BatteryStatus>, HidppError>>,
        ) -> Self {
            Self {
                selected_feature,
                feature_results: RefCell::new(feature_results.into()),
                query_results: RefCell::new(query_results.into()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<TransportCall> {
            self.calls.borrow().clone()
        }
    }

    impl BatteryDevice for FakeTransport {
        fn selected_feature(&self) -> BatteryFeature {
            self.selected_feature
        }

        fn discover_feature(&self, feature_id: u16) -> Result<Option<u8>, HidppError> {
            self.calls
                .borrow_mut()
                .push(TransportCall::Discover(feature_id));
            self.feature_results
                .borrow_mut()
                .pop_front()
                .expect("fake feature result should be configured")
        }

        fn query_feature(
            &self,
            feature: BatteryFeature,
        ) -> Result<Option<BatteryStatus>, HidppError> {
            self.calls.borrow_mut().push(TransportCall::Query(feature));
            self.query_results
                .borrow_mut()
                .pop_front()
                .expect("fake query result should be configured")
        }
    }

    fn short_response_error() -> HidppError {
        HidppError::Protocol(ProtocolError::ShortResponse {
            minimum: 8,
            actual: 7,
        })
    }

    #[test]
    fn parses_root_feature_index_from_byte_four() {
        let response = [REPORT_ID_SHORT, 1, 0, 0x0F, 7, 0, 0];
        assert_eq!(parse_feature_index(&response), Ok(Some(7)));
    }

    #[test]
    fn treats_zero_root_feature_index_as_not_supported() {
        let response = [REPORT_ID_LONG, 1, 0, 0x0F, 0, 0];
        assert_eq!(parse_feature_index(&response), Ok(None));
    }

    #[test]
    fn parses_unified_battery_level_and_charging_bit() {
        let response = [REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0, 1];
        assert_eq!(
            parse_unified_battery(&response),
            Ok(BatteryStatus {
                level: 78,
                charging: true,
            })
        );
    }

    #[test]
    fn parses_legacy_battery_level_and_charging_bit() {
        let response = [REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0, 1];
        assert_eq!(
            parse_battery_status(&response),
            Ok(BatteryStatus {
                level: 42,
                charging: true,
            })
        );
    }

    #[test]
    fn rejects_unified_battery_response_missing_charging_byte() {
        let response = [REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0];
        assert_eq!(
            parse_unified_battery(&response),
            Err(ProtocolError::ShortResponse {
                minimum: 8,
                actual: 7,
            })
        );
    }

    #[test]
    fn rejects_legacy_battery_response_missing_charging_byte() {
        let response = [REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0];
        assert_eq!(
            parse_battery_status(&response),
            Err(ProtocolError::ShortResponse {
                minimum: 7,
                actual: 6,
            })
        );
    }

    #[test]
    fn rejects_response_for_another_device_index() {
        let response = [REPORT_ID_SHORT, 2, 5, 0x1F, 78, 0, 0, 1];
        assert_eq!(match_report(&response, 1, 5, 0x1F), ReportMatch::Unrelated);
        assert_eq!(match_report(&response, 2, 5, 0x1F), ReportMatch::Reply);
    }

    #[test]
    fn rejects_response_echoing_another_function_or_software_id() {
        let response = [REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0, 1];
        assert_eq!(match_report(&response, 1, 5, 0x0F), ReportMatch::Unrelated);
        assert_eq!(match_report(&response, 1, 5, 0x1E), ReportMatch::Unrelated);
    }

    mod request_timing {
        use super::*;

        fn after(mut step: ScriptStep, elapsed: Duration) -> ScriptStep {
            match &mut step {
                ScriptStep::Read { advance, .. } | ScriptStep::Write { advance, .. } => {
                    *advance = elapsed;
                }
            }
            step
        }

        fn silent_probe(device_index: u8) -> [ScriptStep; 3] {
            [
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, device_index, 0, 0x0F, 0x10, 0x04, 0]),
                ScriptStep::silence(1_500, Duration::from_millis(1_500)),
            ]
        }

        #[test]
        fn continuous_queued_reports_stop_flushing_at_the_window_boundary() {
            for last_read_ms in [1, 2] {
                let stale = [REPORT_ID_SHORT, 1, 5, 0x1F, 8, 0, 0, 0];
                let battery_request = [REPORT_ID_SHORT, 1, 5, 0x1F, 0, 0, 0];
                let io = ScriptedIo::new(vec![
                    after(ScriptStep::flush(&stale), Duration::from_millis(100)),
                    after(ScriptStep::flush(&stale), Duration::from_millis(99)),
                    after(
                        ScriptStep::flush(&stale),
                        Duration::from_millis(last_read_ms),
                    ),
                    ScriptStep::write(&battery_request),
                    ScriptStep::response(1_500, &[REPORT_ID_SHORT, 1, 5, 0x1F, 78, 0, 0, 1]),
                ]);
                let start = io.now();
                let session = BatterySession {
                    io: &io,
                    device_index: 1,
                    battery_feature: BatteryFeature::Unified(5),
                };

                assert_eq!(
                    read_battery_from(&session).unwrap(),
                    Some(BatteryStatus {
                        level: 78,
                        charging: true,
                    })
                );
                assert_eq!(
                    io.now().duration_since(start),
                    Duration::from_millis(199 + last_read_ms)
                );
                assert_eq!(io.writes(), vec![battery_request]);
                io.assert_finished();
            }
        }

        #[test]
        fn unrelated_reports_consume_the_original_response_wait() {
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
                after(
                    ScriptStep::response(1_500, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]),
                    Duration::from_millis(500),
                ),
                after(
                    ScriptStep::response(1_000, &[REPORT_ID_SHORT, 1, 0, 0x0E, 5, 0, 0]),
                    Duration::from_millis(750),
                ),
                ScriptStep::silence(250, Duration::from_millis(250)),
            ]);
            let start = io.now();

            let result = get_feature_index_on_device(
                &io,
                1,
                DEFAULT_SW_ID,
                FEATURE_UNIFIED_BATTERY,
                RESPONSE_TIMEOUT,
            );

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.now().duration_since(start), Duration::from_millis(1_500));
            io.assert_finished();
        }

        #[test]
        fn fractional_milliseconds_are_truncated_without_ending_the_wait_early() {
            let unrelated = [REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0];
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
                after(
                    ScriptStep::response(2, &unrelated),
                    Duration::from_micros(1_750),
                ),
                after(
                    ScriptStep::response(0, &unrelated),
                    Duration::from_micros(500),
                ),
                ScriptStep::silence(0, Duration::from_micros(250)),
            ]);
            let start = io.now();

            let result = get_feature_index_on_device(
                &io,
                1,
                DEFAULT_SW_ID,
                FEATURE_UNIFIED_BATTERY,
                Duration::from_micros(2_500),
            );

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.now().duration_since(start), Duration::from_micros(2_500));
            io.assert_finished();
        }

        #[test]
        fn zero_duration_ignores_an_unrelated_report_and_returns_silence() {
            let discovery_request = [REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0];
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&discovery_request),
                ScriptStep::response(0, &[REPORT_ID_SHORT, 2, 0, 0x0F, 5, 0, 0]),
            ]);

            let result = get_feature_index_on_device(
                &io,
                1,
                DEFAULT_SW_ID,
                FEATURE_UNIFIED_BATTERY,
                Duration::ZERO,
            );

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.writes(), vec![discovery_request]);
            io.assert_finished();
        }

        #[test]
        fn discovery_overhead_preserves_the_wait_and_later_battery_request_durations() {
            let unified_lookup = [REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0];
            let unified_query = [REPORT_ID_SHORT, 1, 5, 0x1F, 0, 0, 0];
            let legacy_lookup = [REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0, 0];
            let legacy_query = [REPORT_ID_SHORT, 1, 6, 0x0F, 0, 0, 0];
            let io = ScriptedIo::new(vec![
                after(
                    ScriptStep::flush(&[REPORT_ID_SHORT, 1, 0, 0x0F, 99, 0, 0]),
                    Duration::from_millis(150),
                ),
                ScriptStep::flush(&[]),
                after(
                    ScriptStep::write(&unified_lookup),
                    Duration::from_millis(100),
                ),
                after(
                    ScriptStep::response(400, &[REPORT_ID_SHORT, 1, 0, 0x0F, 5, 0, 0]),
                    Duration::from_millis(400),
                ),
                ScriptStep::flush(&[]),
                ScriptStep::write(&unified_query),
                ScriptStep::silence(1_500, Duration::from_millis(1_500)),
                ScriptStep::flush(&[]),
                ScriptStep::write(&legacy_lookup),
                after(
                    ScriptStep::response(1_500, &[REPORT_ID_SHORT, 1, 0, 0x0F, 6, 0, 0]),
                    Duration::from_millis(1_500),
                ),
                ScriptStep::flush(&[]),
                ScriptStep::write(&legacy_query),
                after(
                    ScriptStep::response(1_500, &[REPORT_ID_SHORT, 1, 6, 0x0F, 42, 0, 1]),
                    Duration::from_millis(1_500),
                ),
            ]);
            let start = io.now();
            // Only 400 ms remain for discovery when this candidate starts.
            let deadline = start + Duration::from_millis(400);
            let result = discover(
                1,
                deadline,
                || io.now(),
                |_, device_index| find_battery_feature(&io, device_index, deadline),
            );
            let Ok(Discovery::Found(battery_feature)) = result else {
                panic!("a matching reply at the response deadline should be accepted");
            };
            assert_eq!(battery_feature, BatteryFeature::Unified(5));
            assert_eq!(io.now().duration_since(start), Duration::from_millis(650));
            assert!(io.now() > deadline);

            let session = BatterySession {
                io: &io,
                device_index: 1,
                battery_feature,
            };
            assert_eq!(
                read_battery_from(&session).unwrap(),
                Some(BatteryStatus {
                    level: 42,
                    charging: true,
                })
            );
            assert_eq!(
                io.writes(),
                vec![unified_lookup, unified_query, legacy_lookup, legacy_query]
            );
            assert_eq!(io.now().duration_since(start), Duration::from_millis(5_150));
            io.assert_finished();
        }

        #[test]
        fn discovery_stops_before_the_next_candidate_after_six_seconds_of_silence() {
            let io = ScriptedIo::new([1, 0xFF, 2, 3].into_iter().flat_map(silent_probe).collect());
            let start = io.now();
            let deadline = start + DISCOVERY_BUDGET;

            let result = discover(
                1,
                deadline,
                || io.now(),
                |_, device_index| find_battery_feature(&io, device_index, deadline),
            );

            assert!(matches!(result, Ok(Discovery::NotFound)));
            assert_eq!(io.now().duration_since(start), Duration::from_secs(6));
            assert_eq!(
                io.writes(),
                [1, 0xFF, 2, 3].map(|device_index| [
                    REPORT_ID_SHORT,
                    device_index,
                    0,
                    0x0F,
                    0x10,
                    0x04,
                    0
                ])
            );
            io.assert_finished();
        }

        #[test]
        fn a_probe_can_finish_its_legacy_lookup_after_the_discovery_budget_is_used() {
            let mut steps = [1, 0xFF, 2]
                .into_iter()
                .flat_map(silent_probe)
                .collect::<Vec<_>>();
            let unified_lookup = [REPORT_ID_SHORT, 3, 0, 0x0F, 0x10, 0x04, 0];
            let legacy_lookup = [REPORT_ID_SHORT, 3, 0, 0x0F, 0x10, 0, 0];
            steps.extend([
                ScriptStep::flush(&[]),
                ScriptStep::write(&unified_lookup),
                after(
                    ScriptStep::response(1_500, &[REPORT_ID_SHORT, 3, 0, 0x0F, 0, 0, 0]),
                    Duration::from_millis(1_500),
                ),
                ScriptStep::flush(&[]),
                ScriptStep::write(&legacy_lookup),
                ScriptStep::response(0, &[REPORT_ID_SHORT, 3, 0, 0x0F, 6, 0, 0]),
            ]);
            let io = ScriptedIo::new(steps);
            let deadline = io.now() + DISCOVERY_BUDGET;

            let result = discover(
                1,
                deadline,
                || io.now(),
                |_, device_index| find_battery_feature(&io, device_index, deadline),
            );

            assert!(matches!(
                result,
                Ok(Discovery::Found(BatteryFeature::Legacy(6)))
            ));
            assert_eq!(io.now(), deadline);
            assert_eq!(io.writes().last(), Some(&legacy_lookup.to_vec()));
            io.assert_finished();
        }

        #[test]
        fn discovery_budget_retains_offline_and_error_precedence() {
            for offline in [true, false] {
                let first_reply = if offline {
                    after(
                        ScriptStep::response(1_500, &[REPORT_ID_SHORT, 1, 0x8F, 0, 0x0F, 0x08, 0]),
                        Duration::from_millis(1_500),
                    )
                } else {
                    ScriptStep::silence(1_500, Duration::from_millis(1_500))
                };
                let mut steps = vec![
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
                    first_reply,
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&[REPORT_ID_SHORT, 0xFF, 0, 0x0F, 0x10, 0x04, 0]),
                    after(
                        ScriptStep::response(1_500, &[REPORT_ID_SHORT, 0xFF, 0, 0x0F]),
                        Duration::from_millis(1_500),
                    ),
                ];
                steps.extend([2, 3].into_iter().flat_map(silent_probe));
                let io = ScriptedIo::new(steps);
                let deadline = io.now() + DISCOVERY_BUDGET;

                let result = discover(
                    1,
                    deadline,
                    || io.now(),
                    |_, device_index| find_battery_feature(&io, device_index, deadline),
                );

                if offline {
                    assert!(matches!(result, Ok(Discovery::Offline)));
                } else {
                    assert!(matches!(
                        result,
                        Err(HidppError::Protocol(ProtocolError::ShortResponse {
                            minimum: 5,
                            actual: 4,
                        }))
                    ));
                }
                assert_eq!(io.now(), deadline);
                io.assert_finished();
            }
        }
    }

    #[test]
    fn caps_request_timeout_at_the_response_timeout() {
        let io = ScriptedIo::new(Vec::new());
        assert_eq!(
            request_timeout(&io, io.now() + DISCOVERY_BUDGET),
            RESPONSE_TIMEOUT
        );
        assert_eq!(request_timeout(&io, io.now()), Duration::ZERO);
    }

    #[test]
    fn offline_outranks_an_error_on_another_interface() {
        let io = ScriptedIo::new(Vec::new());
        let result = discover::<()>(
            2,
            io.now() + DISCOVERY_BUDGET,
            || io.now(),
            |interface, _| match interface {
                0 => Ok(ProbeOutcome::Offline),
                _ => Err(HidppError::Hid(HidError::InitializationError)),
            },
        );
        assert!(matches!(result, Ok(Discovery::Offline)));
    }

    #[test]
    fn keeps_probing_past_an_offline_index() {
        let io = ScriptedIo::new(Vec::new());
        let result = discover(
            2,
            io.now() + DISCOVERY_BUDGET,
            || io.now(),
            |interface, device_index| {
                Ok(match (interface, device_index) {
                    (1, 0xFF) => ProbeOutcome::Found("usb"),
                    (0, _) => ProbeOutcome::Offline,
                    _ => ProbeOutcome::Absent,
                })
            },
        );
        assert!(matches!(result, Ok(Discovery::Found("usb"))));
    }

    #[test]
    fn reports_offline_for_receiver_unknown_device_error() {
        // Captured from a LIGHTSPEED receiver while its paired mouse was off.
        let response = [REPORT_ID_SHORT, 1, 0x8F, 0x00, 0x0F, 0x08, 0x00];
        assert_eq!(match_report(&response, 1, 0x00, 0x0F), ReportMatch::Offline);
    }

    #[test]
    fn recognizes_hidpp20_error_for_the_request() {
        let response = [
            REPORT_ID_LONG,
            0xFF,
            0xFF,
            0x05,
            0x1F,
            0x02,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        assert_eq!(
            match_report(&response, 0xFF, 0x05, 0x1F),
            ReportMatch::DeviceError
        );
    }

    #[test]
    fn ignores_error_echoing_another_request() {
        let response = [REPORT_ID_SHORT, 1, 0x8F, 0x00, 0x0F, 0x08, 0x00];
        assert_eq!(
            match_report(&response, 1, 0x05, 0x0F),
            ReportMatch::Unrelated
        );
        assert_eq!(
            match_report(&response, 1, 0x00, 0x1F),
            ReportMatch::Unrelated
        );
    }

    #[test]
    fn uses_the_discovered_feature_without_rediscovery() {
        let status = BatteryStatus {
            level: 78,
            charging: true,
        };
        for selected_feature in [BatteryFeature::Unified(5), BatteryFeature::Legacy(6)] {
            let transport =
                FakeTransport::new(selected_feature, Vec::new(), vec![Ok(Some(status))]);

            let result = read_battery_from(&transport);

            assert!(matches!(result, Ok(Some(found)) if found == status));
            assert_eq!(
                transport.calls(),
                vec![TransportCall::Query(selected_feature)]
            );
        }
    }

    #[test]
    fn unified_protocol_error_discovers_and_queries_legacy() {
        let status = BatteryStatus {
            level: 42,
            charging: false,
        };
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(Some(6))],
            vec![Err(short_response_error()), Ok(Some(status))],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(result, Ok(Some(found)) if found == status));
        assert_eq!(
            transport.calls(),
            vec![
                TransportCall::Query(BatteryFeature::Unified(5)),
                TransportCall::Discover(FEATURE_BATTERY_STATUS),
                TransportCall::Query(BatteryFeature::Legacy(6)),
            ]
        );
    }

    #[test]
    fn missing_unified_response_discovers_and_queries_legacy() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(Some(6))],
            vec![Ok(None), Ok(None)],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(result, Ok(None)));
        assert_eq!(
            transport.calls(),
            vec![
                TransportCall::Query(BatteryFeature::Unified(5)),
                TransportCall::Discover(FEATURE_BATTERY_STATUS),
                TransportCall::Query(BatteryFeature::Legacy(6)),
            ]
        );
    }

    #[test]
    fn preserves_unified_protocol_error_when_legacy_is_unavailable() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(None)],
            vec![Err(short_response_error())],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(
            result,
            Err(HidppError::Protocol(ProtocolError::ShortResponse {
                minimum: 8,
                actual: 7,
            }))
        ));
        assert_eq!(
            transport.calls(),
            vec![
                TransportCall::Query(BatteryFeature::Unified(5)),
                TransportCall::Discover(FEATURE_BATTERY_STATUS),
            ]
        );
    }

    #[test]
    fn preserves_unified_protocol_error_over_a_legacy_protocol_error() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(Some(6))],
            vec![
                Err(short_response_error()),
                Err(HidppError::Protocol(ProtocolError::ShortResponse {
                    minimum: 7,
                    actual: 6,
                })),
            ],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(
            result,
            Err(HidppError::Protocol(ProtocolError::ShortResponse {
                minimum: 8,
                actual: 7,
            }))
        ));
        assert_eq!(
            transport.calls(),
            vec![
                TransportCall::Query(BatteryFeature::Unified(5)),
                TransportCall::Discover(FEATURE_BATTERY_STATUS),
                TransportCall::Query(BatteryFeature::Legacy(6)),
            ]
        );
    }

    /// Transport failures that are not protocol errors; `HidppError` is not
    /// `Clone`, so each case builds a fresh one.
    const TRANSPORT_FAILURES: [fn() -> HidppError; 2] = [
        || HidppError::Hid(HidError::InitializationError),
        || HidppError::ShortWrite {
            expected: 7,
            actual: 3,
        },
    ];

    #[test]
    fn returns_legacy_transport_failure_after_unified_protocol_error() {
        for failure in TRANSPORT_FAILURES {
            let transport = FakeTransport::new(
                BatteryFeature::Unified(5),
                vec![Ok(Some(6))],
                vec![Err(short_response_error()), Err(failure())],
            );

            let result = read_battery_from(&transport);

            assert_eq!(
                result.map_err(|error| error.to_string()),
                Err(failure().to_string())
            );
            assert_eq!(
                transport.calls(),
                vec![
                    TransportCall::Query(BatteryFeature::Unified(5)),
                    TransportCall::Discover(FEATURE_BATTERY_STATUS),
                    TransportCall::Query(BatteryFeature::Legacy(6)),
                ]
            );
        }
    }

    #[test]
    fn does_not_fall_back_after_unified_transport_failure() {
        for failure in TRANSPORT_FAILURES {
            let transport =
                FakeTransport::new(BatteryFeature::Unified(5), Vec::new(), vec![Err(failure())]);

            let result = read_battery_from(&transport);

            assert_eq!(
                result.map_err(|error| error.to_string()),
                Err(failure().to_string())
            );
            assert_eq!(
                transport.calls(),
                vec![TransportCall::Query(BatteryFeature::Unified(5))]
            );
        }
    }

    mod scripted_success {
        use super::*;

        #[test]
        fn discovers_and_reads_unified_battery_from_raw_reports() {
            for (device_index, feature_index, level, charging_byte, charging) in
                [(1, 5, 78, 0x81, true), (0xFF, 9, 42, 0x02, false)]
            {
                let discover_report = [REPORT_ID_SHORT, device_index, 0, 0x0F, 0x10, 0x04, 0];
                let battery_report = [REPORT_ID_SHORT, device_index, feature_index, 0x1F, 0, 0, 0];
                let mut stale_battery = [0; REPORT_BUFFER_LEN];
                stale_battery[..8].copy_from_slice(&[
                    REPORT_ID_LONG,
                    device_index,
                    feature_index,
                    0x1F,
                    8,
                    0,
                    0,
                    0,
                ]);
                let mut battery_response = [0; REPORT_BUFFER_LEN];
                battery_response[..8].copy_from_slice(&[
                    REPORT_ID_LONG,
                    device_index,
                    feature_index,
                    0x1F,
                    level,
                    0,
                    0,
                    charging_byte,
                ]);
                let io = ScriptedIo::new(vec![
                    ScriptStep::flush(&[REPORT_ID_SHORT, device_index, 0, 0x0F, 99, 0, 0]),
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&discover_report),
                    ScriptStep::response(
                        1_500,
                        &[REPORT_ID_SHORT, device_index, 0, 0x0F, feature_index, 0, 0],
                    ),
                    ScriptStep::flush(&stale_battery),
                    ScriptStep::flush(&[]),
                    ScriptStep::write(&battery_report),
                    ScriptStep::response(1_500, &battery_response),
                ]);

                let ProbeOutcome::Found(battery_feature) =
                    find_battery_feature(&io, device_index, io.now() + DISCOVERY_BUDGET).unwrap()
                else {
                    panic!("battery feature should be discovered");
                };
                let session = BatterySession {
                    io: &io,
                    device_index,
                    battery_feature,
                };
                assert_eq!(
                    read_battery_from(&session).unwrap(),
                    Some(BatteryStatus { level, charging })
                );
                assert_eq!(io.writes(), vec![discover_report, battery_report]);
                io.assert_finished();
            }
        }

        #[test]
        fn zero_timeout_response_is_available_after_its_write() {
            let response = [REPORT_ID_SHORT, 1, 0, 0x0F, 5, 0, 0];
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
                ScriptStep::response(0, &response),
            ]);

            let result = get_feature_index_on_device(
                &io,
                1,
                DEFAULT_SW_ID,
                FEATURE_UNIFIED_BATTERY,
                Duration::ZERO,
            );

            assert!(matches!(result, Ok(FeatureLookup::Index(5))));
            assert_eq!(io.calls.borrow().first(), Some(&IoCall::Read(0)));
            assert_eq!(io.calls.borrow().last(), Some(&IoCall::Read(0)));
            io.assert_finished();
        }

        #[test]
        fn expected_silence_advances_virtual_time() {
            let timeout = Duration::from_millis(10);
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
                ScriptStep::silence(10, timeout),
            ]);
            let start = io.now();

            let result = get_feature_index_on_device(
                &io,
                1,
                DEFAULT_SW_ID,
                FEATURE_UNIFIED_BATTERY,
                timeout,
            );

            assert!(matches!(result, Ok(FeatureLookup::Silent)));
            assert_eq!(io.now().duration_since(start), timeout);
            io.assert_finished();
        }

        #[test]
        #[should_panic(expected = "script exhausted during read")]
        fn an_exhausted_script_fails_instead_of_inventing_silence() {
            let io = ScriptedIo::new(vec![
                ScriptStep::flush(&[]),
                ScriptStep::write(&[REPORT_ID_SHORT, 1, 0, 0x0F, 0x10, 0x04, 0]),
            ]);
            let _ = find_battery_feature(&io, 1, io.now() + DISCOVERY_BUDGET);
        }

        #[test]
        #[should_panic(expected = "unexpected read; expected Write")]
        fn an_unexpected_io_operation_fails_explicitly() {
            let io = ScriptedIo::new(vec![ScriptStep::write(&[
                REPORT_ID_SHORT,
                1,
                0,
                0x0F,
                0x10,
                0x04,
                0,
            ])]);
            let _ = find_battery_feature(&io, 1, io.now() + DISCOVERY_BUDGET);
        }

        #[test]
        #[should_panic(expected = "a scripted response requires a preceding write")]
        fn a_script_cannot_expose_a_response_before_its_write() {
            let _ = ScriptedIo::new(vec![ScriptStep::response(
                0,
                &[REPORT_ID_SHORT, 1, 0, 0x0F, 5, 0, 0],
            )]);
        }
    }
}
