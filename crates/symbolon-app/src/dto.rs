//! What the UI receives and sends: small serializable views of core types, exported to
//! TypeScript by tauri-specta. 64-bit numbers are exported as `number` (exact up to 2^53).

use serde::{Deserialize, Serialize};
use specta_typescript::Number;
use symbolon_core::catalog::{HashPolicy, Location, LocationRoot};
use symbolon_core::query::{LocationStats, ScanInfo};
use symbolon_core::scan::ScanStatus;
use symbolon_core::transport::android::{Device, DeviceState};

/// Milliseconds since the Unix epoch, for timestamps sent to the UI.
pub(crate) fn epoch_ms(time: std::time::SystemTime) -> i64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Where a location's files live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum LocationKind {
    Folder,
    Android,
    Imported,
    Foreign,
}

/// Whether a location can be reached now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub(crate) enum LocationStatus {
    Connected,
    Missing,
    Moved {
        to: String,
    },
    Ambiguous {
        candidates: Vec<String>,
    },
    OtherDevice,
    DeviceNotAttached,
    /// Imported or foreign: compared, never scanned.
    NotScannable,
}

/// When to hash a location's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub(crate) enum HashPolicyView {
    Lazy,
    Eager,
}

impl From<HashPolicy> for HashPolicyView {
    fn from(policy: HashPolicy) -> Self {
        match policy {
            HashPolicy::Eager => Self::Eager,
            HashPolicy::Lazy => Self::Lazy,
        }
    }
}

impl From<HashPolicyView> for HashPolicy {
    fn from(policy: HashPolicyView) -> Self {
        match policy {
            HashPolicyView::Lazy => Self::Lazy,
            HashPolicyView::Eager => Self::Eager,
        }
    }
}

/// How a scan ended, or that it still runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum ScanStatusView {
    Running,
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

impl From<ScanStatus> for ScanStatusView {
    fn from(status: ScanStatus) -> Self {
        match status {
            ScanStatus::Running => Self::Running,
            ScanStatus::Completed => Self::Completed,
            ScanStatus::Cancelled => Self::Cancelled,
            ScanStatus::Failed => Self::Failed,
            ScanStatus::Interrupted => Self::Interrupted,
        }
    }
}

/// One recorded scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScanBrief {
    #[specta(type = Number)]
    pub(crate) scan_id: i64,
    pub(crate) status: ScanStatusView,
    #[specta(type = Number)]
    pub(crate) started_ms: i64,
    #[specta(type = Option<Number>)]
    pub(crate) finished_ms: Option<i64>,
    #[specta(type = Number)]
    pub(crate) files: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
    #[specta(type = Number)]
    pub(crate) warnings: u64,
    pub(crate) error: Option<String>,
}

impl From<&ScanInfo> for ScanBrief {
    fn from(scan: &ScanInfo) -> Self {
        Self {
            scan_id: scan.id.get(),
            status: scan.status.into(),
            started_ms: epoch_ms(scan.started_at),
            finished_ms: scan.finished_at.map(epoch_ms),
            files: scan.counts.files,
            bytes: scan.counts.bytes,
            warnings: scan.counts.warnings,
            error: scan.error.clone(),
        }
    }
}

/// What a location's catalog holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatsView {
    #[specta(type = Number)]
    pub(crate) files: u64,
    #[specta(type = Number)]
    pub(crate) dirs: u64,
    #[specta(type = Number)]
    pub(crate) symlinks: u64,
    #[specta(type = Number)]
    pub(crate) bytes: u64,
    #[specta(type = Number)]
    pub(crate) verified_files: u64,
    #[specta(type = Number)]
    pub(crate) partial_files: u64,
}

impl From<LocationStats> for StatsView {
    fn from(stats: LocationStats) -> Self {
        Self {
            files: stats.files,
            dirs: stats.dirs,
            symlinks: stats.symlinks,
            bytes: stats.bytes,
            verified_files: stats.verified_files,
            partial_files: stats.partial_files,
        }
    }
}

/// A saved location as the Locations screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationView {
    pub(crate) label: String,
    /// The root for display (lossy for paths that are not valid text).
    pub(crate) root: String,
    pub(crate) kind: LocationKind,
    pub(crate) status: LocationStatus,
    pub(crate) hash_policy: HashPolicyView,
    pub(crate) hash_algo: String,
    pub(crate) last_scan: Option<ScanBrief>,
    pub(crate) stats: StatsView,
}

impl LocationView {
    pub(crate) fn kind_of(location: &Location) -> LocationKind {
        match location.root {
            LocationRoot::Local(_) => LocationKind::Folder,
            LocationRoot::Android { .. } => LocationKind::Android,
            LocationRoot::Imported { .. } => LocationKind::Imported,
            _ => LocationKind::Foreign,
        }
    }
}

/// Where a new location's files are.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub(crate) enum NewLocationKind {
    Folder { path: String },
    Android { serial: String, path: String },
}

/// The Add location form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewLocationRequest {
    pub(crate) label: String,
    pub(crate) kind: NewLocationKind,
    pub(crate) hash_policy: HashPolicyView,
    /// `blake3`, `xxh3` or `sha256`; the default when absent.
    pub(crate) hash_algo: Option<String>,
    pub(crate) ignore_patterns: Vec<String>,
}

/// A phone adb can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceView {
    pub(crate) serial: String,
    pub(crate) model: Option<String>,
    pub(crate) ready: bool,
    /// What to do about a phone that is not ready.
    pub(crate) hint: Option<String>,
}

impl From<Device> for DeviceView {
    fn from(device: Device) -> Self {
        Self {
            hint: device.state.hint(),
            ready: device.state == DeviceState::Device,
            model: device.model,
            serial: device.serial,
        }
    }
}
