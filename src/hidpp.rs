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
fn flush(device: &HidDevice) -> Result<(), HidppError> {
    let deadline = Instant::now() + FLUSH_WINDOW;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    while Instant::now() < deadline {
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
fn request_timeout(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .min(RESPONSE_TIMEOUT)
}

fn send_short(
    device: &HidDevice,
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
    device: &HidDevice,
    device_index: u8,
    feature_index: u8,
    function_byte: u8,
    timeout: Duration,
) -> Result<Reply, HidppError> {
    let deadline = Instant::now() + timeout;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
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
        if Instant::now() >= deadline {
            return Ok(Reply::Silence);
        }
    }
}

/// Send one short request and wait for its reply.
fn request(
    device: &HidDevice,
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
    device: &HidDevice,
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
    device: &HidDevice,
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
    let lookup = get_feature_index_on_device(
        &transport.device,
        transport.device_index,
        DEFAULT_SW_ID,
        feature_id,
        RESPONSE_TIMEOUT,
    )?;
    Ok(match lookup {
        FeatureLookup::Index(feature_index) => Some(feature_index),
        FeatureLookup::Unsupported | FeatureLookup::Offline | FeatureLookup::Silent => None,
    })
}

impl BatteryDevice for HidppTransport {
    fn selected_feature(&self) -> BatteryFeature {
        self.battery_feature
    }

    fn discover_feature(&self, feature_id: u16) -> Result<Option<u8>, HidppError> {
        get_feature_index(self, feature_id)
    }

    fn query_feature(&self, feature: BatteryFeature) -> Result<Option<BatteryStatus>, HidppError> {
        let (feature_index, function, parser) = match feature {
            BatteryFeature::Unified(index) => (index, 0x01, parse_unified_battery as _),
            BatteryFeature::Legacy(index) => (index, 0x00, parse_battery_status as _),
        };
        query_battery_on_device(
            &self.device,
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
    device: &HidDevice,
    device_index: u8,
    deadline: Instant,
) -> Result<ProbeOutcome<BatteryFeature>, HidppError> {
    let unified = get_feature_index_on_device(
        device,
        device_index,
        DEFAULT_SW_ID,
        FEATURE_UNIFIED_BATTERY,
        request_timeout(deadline),
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
        request_timeout(deadline),
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
    mut probe: impl FnMut(usize, u8) -> Result<ProbeOutcome<D>, HidppError>,
) -> Result<Discovery<D>, HidppError> {
    let mut offline = false;
    let mut last_error = None;
    'probe: for &device_index in DEVICE_INDEX_CANDIDATES {
        for interface in 0..interface_count {
            if Instant::now() >= deadline {
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

    discover(candidates.len(), deadline, |interface, device_index| {
        let Some(device) = probe_device(api, candidates[interface], &mut slots[interface])? else {
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
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

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

    #[test]
    fn builds_function_byte_from_function_and_software_id() {
        assert_eq!(function_byte(0x01, DEFAULT_SW_ID), 0x1F);
        assert_eq!(function_byte(0x00, DEFAULT_SW_ID), 0x0F);
    }

    #[test]
    fn caps_request_timeout_at_the_response_timeout() {
        assert_eq!(
            request_timeout(Instant::now() + DISCOVERY_BUDGET),
            RESPONSE_TIMEOUT
        );
        assert_eq!(request_timeout(Instant::now()), Duration::ZERO);
    }

    fn far_deadline() -> Instant {
        Instant::now() + DISCOVERY_BUDGET
    }

    #[test]
    fn reports_offline_when_every_probe_finds_the_mouse_offline() {
        let result = discover::<()>(2, far_deadline(), |_, _| Ok(ProbeOutcome::Offline));
        assert!(matches!(result, Ok(Discovery::Offline)));
    }

    #[test]
    fn offline_outranks_an_error_on_another_interface() {
        let result = discover::<()>(2, far_deadline(), |interface, _| match interface {
            0 => Ok(ProbeOutcome::Offline),
            _ => Err(HidppError::Hid(HidError::InitializationError)),
        });
        assert!(matches!(result, Ok(Discovery::Offline)));
    }

    #[test]
    fn keeps_probing_past_an_offline_index() {
        let result = discover(2, far_deadline(), |interface, device_index| {
            Ok(match (interface, device_index) {
                (1, 0xFF) => ProbeOutcome::Found("usb"),
                (0, _) => ProbeOutcome::Offline,
                _ => ProbeOutcome::Absent,
            })
        });
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
    fn ignores_error_response_for_another_device_index() {
        let response = [REPORT_ID_SHORT, 2, 0x8F, 0x00, 0x0F, 0x08, 0x00];
        assert_eq!(
            match_report(&response, 1, 0x00, 0x0F),
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
    fn returns_legacy_transport_error_after_unified_protocol_error() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(Some(6))],
            vec![
                Err(short_response_error()),
                Err(HidppError::Hid(HidError::InitializationError)),
            ],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(
            result,
            Err(HidppError::Hid(HidError::InitializationError))
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

    #[test]
    fn returns_legacy_short_write_after_unified_protocol_error() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            vec![Ok(Some(6))],
            vec![
                Err(short_response_error()),
                Err(HidppError::ShortWrite {
                    expected: 7,
                    actual: 3,
                }),
            ],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(
            result,
            Err(HidppError::ShortWrite {
                expected: 7,
                actual: 3,
            })
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

    #[test]
    fn does_not_fall_back_after_unified_hid_error() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            Vec::new(),
            vec![Err(HidppError::Hid(HidError::InitializationError))],
        );

        assert!(matches!(
            read_battery_from(&transport),
            Err(HidppError::Hid(HidError::InitializationError))
        ));
        assert_eq!(
            transport.calls(),
            vec![TransportCall::Query(BatteryFeature::Unified(5))]
        );
    }

    #[test]
    fn does_not_fall_back_after_unified_transport_failure() {
        let transport = FakeTransport::new(
            BatteryFeature::Unified(5),
            Vec::new(),
            vec![Err(HidppError::ShortWrite {
                expected: 7,
                actual: 3,
            })],
        );

        let result = read_battery_from(&transport);

        assert!(matches!(
            result,
            Err(HidppError::ShortWrite {
                expected: 7,
                actual: 3,
            })
        ));
        assert_eq!(
            transport.calls(),
            vec![TransportCall::Query(BatteryFeature::Unified(5))]
        );
    }
}
