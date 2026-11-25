//! `symbolon devices`: phones adb can see, and what to do about the ones that are not ready.

use serde::Serialize;
use symbolon_core::transport::android::{AndroidTransport as _, SystemAdb};

use crate::cli::Global;
use crate::exit::Exit;
use crate::output::{self, Record};

/// One device as adb reports it.
#[derive(Debug, Serialize)]
struct DeviceRecord {
    serial: String,
    /// `device` when ready; otherwise adb's state, such as `unauthorized` or `offline`.
    state: String,
    model: Option<String>,
    product: Option<String>,
    /// What to do to make it ready; absent when it is.
    hint: Option<String>,
}

impl Record for DeviceRecord {
    const FIELDS: &'static [&'static str] = &["serial", "state", "model", "product", "hint"];
    const HEADERS: &'static [&'static str] = &["SERIAL", "STATE", "MODEL", "HINT"];

    fn cells(&self) -> Vec<String> {
        vec![
            self.serial.clone(),
            self.state.clone(),
            self.model.clone().unwrap_or_default(),
            self.hint.clone().unwrap_or_default(),
        ]
    }
}

pub(super) fn run(global: &Global) -> anyhow::Result<Exit> {
    let records: Vec<DeviceRecord> = SystemAdb::new()
        .devices()?
        .into_iter()
        .map(|device| DeviceRecord {
            hint: device.state.hint(),
            state: device.state.to_string(),
            serial: device.serial,
            model: device.model,
            product: device.product,
        })
        .collect();
    if records.is_empty() && !global.quiet {
        eprintln!(
            "No phones found. Connect one with USB debugging on (Settings > Developer options)."
        );
    }
    output::list(global.format(), "devices", &records)?;
    Ok(Exit::Success)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_match_the_record() {
        output::assert_fields(&DeviceRecord {
            serial: String::new(),
            state: String::new(),
            model: None,
            product: None,
            hint: None,
        });
    }
}
