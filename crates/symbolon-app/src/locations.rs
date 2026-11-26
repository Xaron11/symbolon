//! Locations and devices: the Locations screen's reads and edits.

use std::path::PathBuf;

use symbolon_core::catalog::{Location, LocationRoot, NewLocation, Whereabouts};
use symbolon_core::hash::HashAlgo;
use symbolon_core::query::{Page, Query};

use crate::dto::{
    DeviceView, HashPolicyView, LocationKind, LocationStatus, LocationView, NewLocationKind,
    NewLocationRequest, ScanBrief, StatsView,
};
use crate::error::{ApiError, ErrorKind};
use crate::state::AppState;

impl AppState {
    /// Every location, by label.
    pub(crate) fn locations_list(&self) -> Result<Vec<LocationView>, ApiError> {
        let query = Query::open(&self.catalog)?;
        self.catalog
            .locations()?
            .iter()
            .map(|location| self.location_view(&query, location))
            .collect()
    }

    pub(crate) fn location_add(
        &self,
        request: NewLocationRequest,
    ) -> Result<LocationView, ApiError> {
        let mut new = match request.kind {
            NewLocationKind::Folder { path } => {
                NewLocation::local(request.label, PathBuf::from(path))
            }
            NewLocationKind::Android { serial, path } => {
                NewLocation::android(request.label, serial, path)
            }
        }
        .hash_policy(request.hash_policy.into())
        .ignore_patterns(request.ignore_patterns);
        if let Some(algo) = request.hash_algo {
            let algo = algo.parse::<HashAlgo>().map_err(|_| {
                ApiError::new(
                    ErrorKind::InvalidInput,
                    format!("unknown hash algorithm {algo:?}"),
                )
            })?;
            new = new.hash_algo(algo);
        }
        let location = self.catalog.add_location(new)?;
        self.location_view(&Query::open(&self.catalog)?, &location)
    }

    pub(crate) fn location_rename(
        &self,
        label: &str,
        new_label: &str,
    ) -> Result<LocationView, ApiError> {
        let location = self.catalog.rename(label, new_label)?;
        self.location_view(&Query::open(&self.catalog)?, &location)
    }

    pub(crate) fn location_remove(&self, label: &str) -> Result<(), ApiError> {
        if self.scans.is_running(label) {
            return Err(ApiError::new(
                ErrorKind::AlreadyScanning,
                "Stop the scan first.",
            ));
        }
        Ok(self.catalog.remove_location(label)?)
    }

    pub(crate) fn location_set_hash_policy(
        &self,
        label: &str,
        policy: HashPolicyView,
    ) -> Result<LocationView, ApiError> {
        let location = self.catalog.set_hash_policy(label, policy.into())?;
        self.location_view(&Query::open(&self.catalog)?, &location)
    }

    /// Points a moved location at where its device is now, or at candidate `choice` of an
    /// ambiguous one. The UI sends no path back: paths are found again here, so none passes
    /// through display text.
    pub(crate) fn location_follow_move(
        &self,
        label: &str,
        choice: Option<u32>,
    ) -> Result<LocationView, ApiError> {
        let to = match (self.catalog.locate(label)?, choice) {
            (Whereabouts::Moved { to, .. }, _) => to,
            (Whereabouts::Ambiguous { candidates, .. }, Some(choice)) => candidates
                .into_iter()
                .nth(choice as usize)
                .ok_or_else(|| ApiError::new(ErrorKind::InvalidInput, "no such candidate"))?,
            _ => {
                return Err(ApiError::new(
                    ErrorKind::InvalidInput,
                    format!("location {label:?} has not moved"),
                ));
            }
        };
        let location = self.catalog.relocate(label, &to)?;
        self.location_view(&Query::open(&self.catalog)?, &location)
    }

    pub(crate) fn location_stats(&self, label: &str) -> Result<StatsView, ApiError> {
        Ok(Query::open(&self.catalog)?.location_stats(label)?.into())
    }

    /// Phones adb can see.
    pub(crate) fn devices_list(&self) -> Result<Vec<DeviceView>, ApiError> {
        Ok(self
            .transport
            .devices()?
            .into_iter()
            .map(DeviceView::from)
            .collect())
    }

    fn location_view(&self, query: &Query, location: &Location) -> Result<LocationView, ApiError> {
        let last_scan = query
            .scans(&location.label, &Page::first(1))?
            .items
            .first()
            .map(ScanBrief::from);
        Ok(LocationView {
            label: location.label.clone(),
            root: location.root.display(),
            kind: LocationView::kind_of(location),
            status: self.status(location)?,
            hash_policy: location.hash_policy.into(),
            hash_algo: location.hash_algo.to_string(),
            last_scan,
            stats: query.location_stats(&location.label)?.into(),
        })
    }

    fn status(&self, location: &Location) -> Result<LocationStatus, ApiError> {
        let android = matches!(location.root, LocationRoot::Android { .. });
        Ok(match self.catalog.locate(&location.label)? {
            Whereabouts::Here(_) => LocationStatus::Connected,
            Whereabouts::Moved { to, .. } => LocationStatus::Moved {
                to: to.display().to_string(),
            },
            Whereabouts::Ambiguous { candidates, .. } => LocationStatus::Ambiguous {
                candidates: candidates.iter().map(|c| c.display().to_string()).collect(),
            },
            Whereabouts::OtherDevice { .. } => LocationStatus::OtherDevice,
            Whereabouts::Missing(_) if android => LocationStatus::DeviceNotAttached,
            Whereabouts::Missing(_) => LocationStatus::Missing,
            _ => match LocationView::kind_of(location) {
                LocationKind::Imported | LocationKind::Foreign => LocationStatus::NotScannable,
                _ => LocationStatus::Missing,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use symbolon_core::catalog::ImportOptions;
    use symbolon_core::scan::{ScanOptions, Scanner};

    use super::*;
    use crate::state::testing::{devices, plugged, state};

    fn folder(label: &str, path: &std::path::Path) -> NewLocationRequest {
        NewLocationRequest {
            label: label.into(),
            kind: NewLocationKind::Folder {
                path: path.to_string_lossy().into_owned(),
            },
            hash_policy: HashPolicyView::Lazy,
            hash_algo: None,
            ignore_patterns: Vec::new(),
        }
    }

    #[test]
    fn add_folder_then_list_shows_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("photos");
        fs::create_dir(&root).unwrap();
        let s = state(dir.path(), Vec::new());
        let added = s.location_add(folder("Photos", &root)).unwrap();
        assert_eq!(added.kind, LocationKind::Folder);
        let list = s.locations_list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "Photos");
        assert_eq!(list[0].status, LocationStatus::Connected);
        assert_eq!(list[0].hash_algo, "blake3");
        assert!(list[0].last_scan.is_none());
    }

    #[test]
    fn add_duplicate_label_is_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Photos", dir.path())).unwrap();
        let err = s.location_add(folder("Photos", dir.path())).unwrap_err();
        assert_eq!(err.kind, ErrorKind::AlreadyExists);
    }

    #[test]
    fn unknown_algorithm_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        let mut request = folder("Photos", dir.path());
        request.hash_algo = Some("md5".into());
        assert_eq!(
            s.location_add(request).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn rename_and_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Photos", dir.path())).unwrap();
        assert_eq!(s.location_rename("Photos", "Pics").unwrap().label, "Pics");
        s.location_remove("Pics").unwrap();
        assert_eq!(s.locations_list().unwrap(), []);
    }

    #[test]
    fn remove_unknown_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        assert_eq!(
            s.location_remove("nope").unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[test]
    fn missing_root_status_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("stick");
        fs::create_dir(&root).unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Stick", &root)).unwrap();
        fs::remove_dir(&root).unwrap();
        assert_eq!(
            s.locations_list().unwrap()[0].status,
            LocationStatus::Missing
        );
    }

    #[test]
    fn set_hash_policy_changes_view() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Photos", dir.path())).unwrap();
        let view = s
            .location_set_hash_policy("Photos", HashPolicyView::Eager)
            .unwrap();
        assert_eq!(view.hash_policy, HashPolicyView::Eager);
    }

    #[test]
    fn last_scan_and_stats_after_a_scan() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("photos");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.jpg"), "abc").unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Photos", &root)).unwrap();
        Scanner::start(&s.catalog, "Photos", ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
        let view = &s.locations_list().unwrap()[0];
        assert_eq!(view.last_scan.as_ref().unwrap().files, 1);
        assert_eq!(view.stats.files, 1);
        assert_eq!(s.location_stats("Photos").unwrap().bytes, 3);
    }

    #[test]
    fn imported_location_is_not_scannable_status() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("photos");
        fs::create_dir(&root).unwrap();
        let other = tempfile::tempdir().unwrap();
        let source = state(other.path(), Vec::new());
        source.location_add(folder("Photos", &root)).unwrap();
        Scanner::start(&source.catalog, "Photos", ScanOptions::default())
            .unwrap()
            .wait()
            .unwrap();
        let export = other.path().join("photos.symbolon");
        source.catalog.export("Photos", &export).unwrap();
        let s = state(dir.path(), Vec::new());
        s.catalog
            .import(&export, &ImportOptions::default())
            .unwrap();
        let view = &s.locations_list().unwrap()[0];
        assert_eq!(view.kind, LocationKind::Imported);
        assert_eq!(view.status, LocationStatus::NotScannable);
    }

    #[test]
    fn android_location_without_its_phone_is_not_attached() {
        let dir = tempfile::tempdir().unwrap();
        let (s, plugged) = plugged(dir.path(), devices("ABC123 device model:Pixel\n"));
        let request = NewLocationRequest {
            kind: NewLocationKind::Android {
                serial: "ABC123".into(),
                path: "/sdcard/DCIM".into(),
            },
            ..folder("Phone", dir.path())
        };
        s.location_add(request).unwrap();
        assert_eq!(
            s.locations_list().unwrap()[0].status,
            LocationStatus::Connected
        );
        *plugged.lock().unwrap() = devices("OTHER device model:Pixel\n");
        let view = &s.locations_list().unwrap()[0];
        assert_eq!(view.kind, LocationKind::Android);
        assert_eq!(view.status, LocationStatus::DeviceNotAttached);
    }

    #[test]
    fn devices_report_readiness_and_hints() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(
            dir.path(),
            devices("A1 device model:Pixel_10\nB2 unauthorized usb:1-1\n"),
        );
        let list = s.devices_list().unwrap();
        assert!(list[0].ready);
        assert_eq!(list[0].model.as_deref(), Some("Pixel_10"));
        assert!(!list[1].ready);
        assert!(list[1].hint.is_some());
    }

    #[test]
    fn follow_move_without_a_move_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let s = state(dir.path(), Vec::new());
        s.location_add(folder("Photos", dir.path())).unwrap();
        assert_eq!(
            s.location_follow_move("Photos", None).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }
}
