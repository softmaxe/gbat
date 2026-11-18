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
/// Stop starting new probes after this budget. A probe already in progress can
/// finish, and reading a discovered battery feature has its own timeout.
const DISCOVERY_BUDGET: Duration = Duration::from_secs(6);

#[derive(Debug, PartialEq, Eq)]
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
            Self::Hid(error) => {
                write!(formatter, "HID access failed: {error}")?;
                #[cfg(target_os = "linux")]
                if permission_denied(error) {
                    write!(
                        formatter,
                        ". On Linux, read battery status with sudo: sudo /path/to/gbat."
                    )?;
                }
                Ok(())
            }
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

#[cfg(target_os = "linux")]
fn permission_denied(error: &HidError) -> bool {
    match error {
        HidError::IoError { error } => error.kind() == std::io::ErrorKind::PermissionDenied,
        // hidapi's native backend wraps open errors in text, but preserves errno.
        HidError::HidApiError { message } => message.ends_with("(os error 13)"),
        _ => false,
    }
}

/// One report read from a HID interface.
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
pub enum Reading {
    Status(BatteryStatus),
    /// A device index exposed a battery feature, but reading it produced no
    /// status.
    NoStatus,
    /// A receiver answered, but reported no connected mouse.
    Offline,
    /// No candidate reported a battery feature or an offline device.
    NotFound,
}

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
    fn write(&self, report: &[u8]) -> Result<(), HidError> {
        let sent = HidDevice::write(self, report)?;
        if sent != report.len() {
            return Err(HidError::IncompleteSendError {
                sent,
                all: report.len(),
            });
        }
        Ok(())
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
            Reply::Response(report) => {
                let bytes = report.bytes();
                ensure_length(bytes, 5)?;
                match bytes[4] {
                    0 => FeatureLookup::Unsupported,
                    index => FeatureLookup::Index(index),
                }
            }
            Reply::DeviceError => FeatureLookup::Unsupported,
            Reply::Offline => FeatureLookup::Offline,
            Reply::Silence => FeatureLookup::Silent,
        })
    }

    fn find_battery_feature(&self, deadline: Instant) -> Result<Discovery, HidppError> {
        let lookup = |feature_id| {
            let timeout = deadline
                .saturating_duration_since(self.io.now())
                .min(RESPONSE_TIMEOUT);
            self.lookup_feature(feature_id, timeout)
        };
        match lookup(FEATURE_UNIFIED_BATTERY)? {
            FeatureLookup::Index(feature_index) => {
                return Ok(Discovery::Found(BatteryFeature::Unified(feature_index)))
            }
            FeatureLookup::Offline => return Ok(Discovery::Offline),
            // Every HID++ 2.0 device answers root feature lookups, so silence means
            // this index is absent and the legacy lookup would only wait again.
            FeatureLookup::Silent => return Ok(Discovery::NotFound),
            FeatureLookup::Unsupported => {}
        }

        Ok(match lookup(FEATURE_BATTERY_STATUS)? {
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
    // Linux can enumerate several HID++ usages for the same hidraw node.
    // Opening it once is enough to receive every report on that interface.
    #[cfg(target_os = "linux")]
    candidates.dedup_by(|left, right| left.path() == right.path());
    candidates
}

/// Read the battery status through the first candidate HID interface and
/// device index that expose a battery feature.
pub fn read_battery(api: &HidApi) -> Result<Reading, HidppError> {
    let interfaces = candidate_interfaces(api)
        .into_iter()
        .map(|info| move || api.open_path(info.path()));
    read_battery_from(interfaces, Instant::now)
}

/// A HID interface, opened on first use and reused across device indices.
struct Slot<Open, Io> {
    opener: Option<Open>,
    io: Option<Io>,
}

impl<Open: FnOnce() -> Result<Io, HidError>, Io> Slot<Open, Io> {
    /// The open HID interface, or `None` if it failed to open. Only the first
    /// failed open reports its error; later probes skip the slot.
    fn open(&mut self) -> Result<Option<&Io>, HidError> {
        if let Some(open) = self.opener.take() {
            self.io = Some(open()?);
        }
        Ok(self.io.as_ref())
    }
}

/// Probe index-major, so the likely device indices on every HID interface come
/// before the unlikely ones on the first, then read the first battery feature
/// found. `interfaces` open the candidate HID interfaces in probe order.
fn read_battery_from<Io: RequestIo>(
    interfaces: impl IntoIterator<Item = impl FnOnce() -> Result<Io, HidError>>,
    now: impl Fn() -> Instant,
) -> Result<Reading, HidppError> {
    let mut slots = interfaces
        .into_iter()
        .map(|open| Slot {
            opener: Some(open),
            io: None,
        })
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
                    last_error = Some(error.into());
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
mod tests;
