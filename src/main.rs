use std::process::ExitCode;

use gbat::hidpp::{open_first_working_transport, read_battery, BatteryStatus};
use hidapi::HidApi;

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{}", version_text());
        return ExitCode::SUCCESS;
    }

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

const PACKAGE_NAME: &str = "gbat";

fn version_text() -> String {
    format!("{PACKAGE_NAME} {}", env!("CARGO_PKG_VERSION"))
}

fn run() -> Result<(), String> {
    let api = HidApi::new().map_err(|error| format!("Could not initialize HID access: {error}"))?;
    let transport = match open_first_working_transport(&api) {
        Ok(Some(transport)) => transport,
        Ok(None) => {
            return Err(String::from(
                "No responsive Logitech HID++ interface found. Connect the GPW2 through its LIGHTSPEED receiver or USB, wake it, and retry.",
            ))
        }
        Err(error) => return Err(format!("Could not probe Logitech HID++ interface: {error}")),
    };
    let status = read_battery(&transport)
        .map_err(|error| format!("Could not read battery level: {error}"))?
        .ok_or_else(|| {
            String::from("Could not read battery level. Wake the mouse up and retry.")
        })?;

    println!("{}", format_status(status));
    Ok(())
}

fn format_status(status: BatteryStatus) -> String {
    let level = status.level.min(100);
    let suffix = if status.charging { " (charging)" } else { "" };
    format!("Battery: {level}%{suffix}")
}

#[cfg(test)]
mod tests {
    use super::{format_status, version_text};
    use gbat::hidpp::BatteryStatus;

    #[test]
    fn formats_battery_status_for_terminal_and_raycast() {
        assert_eq!(
            format_status(BatteryStatus {
                level: 78,
                charging: false,
            }),
            "Battery: 78%"
        );
        assert_eq!(
            format_status(BatteryStatus {
                level: 42,
                charging: true,
            }),
            "Battery: 42% (charging)"
        );
    }

    #[test]
    fn clamps_invalid_reported_levels() {
        assert_eq!(
            format_status(BatteryStatus {
                level: u8::MAX,
                charging: false,
            }),
            "Battery: 100%"
        );
    }

    #[test]
    fn formats_version_for_release_and_homebrew_checks() {
        assert_eq!(
            version_text(),
            concat!("gbat ", env!("CARGO_PKG_VERSION"))
        );
    }
}
