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
pub const REPORT_ID_ERROR: u8 = 0x8F;
pub const DEFAULT_SW_ID: u8 = 0x0F;

pub const DEVICE_INDEX_CANDIDATES: &[u8] = &[1, 2, 3, 4, 5, 6, 0xFF];

const REPORT_BUFFER_LEN: usize = 20;
const FLUSH_TIMEOUT_MS: i32 = 50;
const FLUSH_WINDOW: Duration = Duration::from_millis(200);
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(1_500);
const RESPONSE_READ_SLICE_MS: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    pub level: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    EmptyResponse,
    ErrorReport,
    UnexpectedReportId(u8),
    ShortResponse { minimum: usize, actual: usize },
    FeatureMismatch { expected: u8, actual: u8 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyResponse => formatter.write_str("empty HID++ response"),
            Self::ErrorReport => formatter.write_str("device returned a HID++ error report"),
            Self::UnexpectedReportId(report_id) => {
                write!(formatter, "unexpected HID++ report ID 0x{report_id:02X}")
            }
            Self::ShortResponse { minimum, actual } => write!(
                formatter,
                "short HID++ response: expected at least {minimum} bytes, got {actual}"
            ),
            Self::FeatureMismatch { expected, actual } => write!(
                formatter,
                "HID++ response feature mismatch: expected 0x{expected:02X}, got 0x{actual:02X}"
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
    validate_response(response, Some(FEATURE_ROOT as u8), 5)?;
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
    validate_response(response, None, charging_offset + 1)?;
    Ok(BatteryStatus {
        level: response[4],
        charging: response[charging_offset] & 0x01 != 0,
    })
}

fn validate_response(
    response: &[u8],
    expected_feature: Option<u8>,
    minimum_length: usize,
) -> Result<(), ProtocolError> {
    let report_id = response
        .first()
        .copied()
        .ok_or(ProtocolError::EmptyResponse)?;
    if report_id == REPORT_ID_ERROR {
        return Err(ProtocolError::ErrorReport);
    }
    if !matches!(report_id, REPORT_ID_SHORT | REPORT_ID_LONG) {
        return Err(ProtocolError::UnexpectedReportId(report_id));
    }
    if response.len() < 3 {
        return Err(ProtocolError::ShortResponse {
            minimum: 3,
            actual: response.len(),
        });
    }
    if let Some(expected) = expected_feature {
        let actual = response[2];
        if actual != expected {
            return Err(ProtocolError::FeatureMismatch { expected, actual });
        }
    }
    if response.len() < minimum_length {
        return Err(ProtocolError::ShortResponse {
            minimum: minimum_length,
            actual: response.len(),
        });
    }
    Ok(())
}

fn flush(device: &HidDevice) -> Result<(), HidppError> {
    let deadline = Instant::now() + FLUSH_WINDOW;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    while Instant::now() < deadline {
        if device.read_timeout(&mut buffer, FLUSH_TIMEOUT_MS)? == 0 {
            break;
        }
    }
    Ok(())
}

fn send_short(
    device: &HidDevice,
    device_index: u8,
    feature_index: u8,
    function: u8,
    sw_id: u8,
    payload: [u8; 3],
) -> Result<(), HidppError> {
    let function_byte = ((function & 0x0F) << 4) | (sw_id & 0x0F);
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

fn response_matches(response: &[u8], device_index: u8, feature_index: u8) -> bool {
    matches!(
        response.first(),
        Some(&REPORT_ID_SHORT) | Some(&REPORT_ID_LONG)
    ) && response.get(1) == Some(&device_index)
        && response.get(2) == Some(&feature_index)
}

fn error_response_matches(response: &[u8], device_index: u8) -> bool {
    response.first() == Some(&REPORT_ID_ERROR) && response.get(1) == Some(&device_index)
}

fn read_response(
    device: &HidDevice,
    device_index: u8,
    feature_index: u8,
) -> Result<Option<Vec<u8>>, HidppError> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let mut buffer = [0_u8; REPORT_BUFFER_LEN];
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout_ms = remaining.as_millis().min(RESPONSE_READ_SLICE_MS as u128) as i32;
        if timeout_ms == 0 {
            break;
        }
        let length = device.read_timeout(&mut buffer, timeout_ms)?;
        if length == 0 {
            continue;
        }
        let response = &buffer[..length];
        if error_response_matches(response, device_index) {
            return Ok(None);
        }
        if response_matches(response, device_index, feature_index) {
            return Ok(Some(response.to_vec()));
        }
    }
    Ok(None)
}

fn get_feature_index_on_device(
    device: &HidDevice,
    device_index: u8,
    sw_id: u8,
    feature_id: u16,
) -> Result<Option<u8>, HidppError> {
    flush(device)?;
    send_short(
        device,
        device_index,
        FEATURE_ROOT as u8,
        0,
        sw_id,
        [(feature_id >> 8) as u8, feature_id as u8, 0],
    )?;
    let response = match read_response(device, device_index, FEATURE_ROOT as u8)? {
        Some(response) => response,
        None => return Ok(None),
    };
    Ok(parse_feature_index(&response)?)
}

fn query_battery_on_device(
    device: &HidDevice,
    device_index: u8,
    sw_id: u8,
    feature_index: u8,
    function: u8,
    parse_response: fn(&[u8]) -> Result<BatteryStatus, ProtocolError>,
) -> Result<Option<BatteryStatus>, HidppError> {
    flush(device)?;
    send_short(device, device_index, feature_index, function, sw_id, [0; 3])?;
    let response = match read_response(device, device_index, feature_index)? {
        Some(response) => response,
        None => return Ok(None),
    };
    Ok(Some(parse_response(&response)?))
}

pub fn get_feature_index(
    transport: &HidppTransport,
    feature_id: u16,
) -> Result<Option<u8>, HidppError> {
    get_feature_index_on_device(
        &transport.device,
        transport.device_index,
        DEFAULT_SW_ID,
        feature_id,
    )
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
) -> Result<Option<BatteryFeature>, HidppError> {
    let unified =
        get_feature_index_on_device(device, device_index, DEFAULT_SW_ID, FEATURE_UNIFIED_BATTERY)?;
    if let Some(feature_index) = unified {
        return Ok(Some(BatteryFeature::Unified(feature_index)));
    }

    let legacy =
        get_feature_index_on_device(device, device_index, DEFAULT_SW_ID, FEATURE_BATTERY_STATUS)?;
    Ok(legacy.map(BatteryFeature::Legacy))
}

pub fn open_first_working_transport(api: &HidApi) -> Result<Option<HidppTransport>, HidppError> {
    let mut last_error = None;
    for candidate in candidate_interfaces(api) {
        let device = match api.open_path(candidate.path()) {
            Ok(device) => device,
            Err(error) => {
                last_error = Some(error.into());
                continue;
            }
        };
        if let Err(error) = device.set_blocking_mode(false) {
            last_error = Some(error.into());
            continue;
        }

        for &device_index in DEVICE_INDEX_CANDIDATES {
            match find_battery_feature(&device, device_index) {
                Ok(Some(battery_feature)) => {
                    return Ok(Some(HidppTransport {
                        device,
                        device_index,
                        battery_feature,
                    }))
                }
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
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
    fn rejects_error_report() {
        let response = [REPORT_ID_ERROR, 1, 0, 0x0F];
        assert_eq!(
            parse_feature_index(&response),
            Err(ProtocolError::ErrorReport)
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
    fn rejects_feature_mismatch() {
        let response = [REPORT_ID_SHORT, 1, 3, 0x0F, 7, 0];
        assert_eq!(
            parse_feature_index(&response),
            Err(ProtocolError::FeatureMismatch {
                expected: FEATURE_ROOT as u8,
                actual: 3,
            })
        );
    }

    #[test]
    fn rejects_response_for_another_device_index() {
        let response = [REPORT_ID_SHORT, 2, 5, 0x1F, 78, 0, 0, 1];
        assert!(!response_matches(&response, 1, 5));
    }

    #[test]
    fn ignores_error_response_for_another_device_index() {
        let response = [REPORT_ID_ERROR, 2, 5, 0x1F, 0x01];
        assert!(!error_response_matches(&response, 1));
        assert!(error_response_matches(&response, 2));
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
