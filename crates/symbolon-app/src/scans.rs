//! Running scans: started from the UI, streamed to it through sinks, and their history.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use specta_typescript::Number;
use symbolon_core::catalog::Catalog;
use symbolon_core::query::{Page, Query};
use symbolon_core::scan::{
    ScanError, ScanEvent, ScanHandle, ScanId, ScanOptions, ScanPhase, ScanProgress, ScanSummary,
    Scanner,
};

use crate::dto::{ScanBrief, ScanStatusView};
use crate::error::{ApiError, ErrorKind};
use crate::sink::Sink;
use crate::state::AppState;

/// Least time between two progress updates of one scan; others are dropped.
pub(crate) const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Least time between two "results changed" notices to browsers of a scanned location.
pub(crate) const BROWSE_INTERVAL: Duration = Duration::from_secs(1);

/// A location's catalog changed under a browser: re-read what is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum BrowseChange {
    /// The running scan committed more entries.
    Changed,
    /// The scan ended; nothing more will change.
    Finished,
}

/// What a scan is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum ScanPhaseView {
    Walking,
    Hashing,
}

/// One message about a running scan.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub(crate) enum ScanUpdate {
    Started {
        label: String,
        #[specta(type = Number)]
        scan_id: i64,
    },
    Progress {
        label: String,
        phase: ScanPhaseView,
        #[specta(type = Number)]
        files: u64,
        #[specta(type = Number)]
        dirs: u64,
        #[specta(type = Number)]
        bytes: u64,
        #[specta(type = Number)]
        hashed_files: u64,
        #[specta(type = Number)]
        hashed_bytes: u64,
        #[specta(type = Number)]
        to_hash_files: u64,
        #[specta(type = Number)]
        to_hash_bytes: u64,
        #[specta(type = Number)]
        bytes_per_sec: u64,
        #[specta(type = Number)]
        warnings: u64,
        #[specta(type = Number)]
        elapsed_ms: u64,
    },
    Finished {
        label: String,
        #[specta(type = Number)]
        scan_id: i64,
        status: ScanStatusView,
        #[specta(type = Number)]
        files: u64,
        #[specta(type = Number)]
        bytes: u64,
        #[specta(type = Number)]
        hashed_files: u64,
        #[specta(type = Number)]
        warnings: u64,
        #[specta(type = Number)]
        elapsed_ms: u64,
    },
    Failed {
        label: String,
        error: ApiError,
    },
}

/// A recorded scan of any location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentScanView {
    pub(crate) label: String,
    pub(crate) scan: ScanBrief,
}

/// A problem a scan recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WarningView {
    /// The entry, for display (lossy).
    pub(crate) path: Option<String>,
    pub(crate) kind: String,
    pub(crate) message: String,
}

/// Lets progress through at most once per interval, and always when the phase changes.
#[derive(Debug)]
pub(crate) struct Throttle {
    interval: Duration,
    last: Option<(Instant, ScanPhase)>,
}

impl Throttle {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    /// Whether progress in `phase` at `now` should be sent; records it if so.
    pub(crate) fn admit(&mut self, now: Instant, phase: ScanPhase) -> bool {
        let admit = self.last.is_none_or(|(at, last_phase)| {
            last_phase != phase || now.duration_since(at) >= self.interval
        });
        if admit {
            self.last = Some((now, phase));
        }
        admit
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Receivers of one scan and the last update they were sent.
#[derive(Default)]
struct Watchers {
    sinks: Vec<Box<dyn Sink<ScanUpdate>>>,
    latest: Option<ScanUpdate>,
}

struct RunningScan {
    /// Taken by the forwarding thread when the scan ends.
    handle: Mutex<Option<ScanHandle>>,
    watchers: Mutex<Watchers>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl RunningScan {
    fn publish(&self, update: ScanUpdate) {
        let mut watchers = lock(&self.watchers);
        watchers.sinks.retain(|sink| sink.send(update.clone()));
        watchers.latest = Some(update);
    }
}

type BrowseSink = Box<dyn Sink<BrowseChange>>;

/// Scans started from the UI, by location label.
#[derive(Default)]
pub(crate) struct ScanRegistry {
    running: Mutex<HashMap<String, Arc<RunningScan>>>,
    /// Browsers of each location, notified while its scans write; kept until they are gone.
    browsers: Mutex<HashMap<String, Vec<BrowseSink>>>,
}

impl ScanRegistry {
    /// Starts scanning `label` with default options and streams its updates to `sink`.
    pub(crate) fn start(
        self: &Arc<Self>,
        catalog: &Catalog,
        label: &str,
        sink: Box<dyn Sink<ScanUpdate>>,
    ) -> Result<(), ApiError> {
        self.start_with(label, sink, || {
            Scanner::start(catalog, label, ScanOptions::default())
        })
    }

    /// Like [`ScanRegistry::start`] with the scan started by `start`.
    pub(crate) fn start_with(
        self: &Arc<Self>,
        label: &str,
        sink: Box<dyn Sink<ScanUpdate>>,
        start: impl FnOnce() -> Result<ScanHandle, ScanError>,
    ) -> Result<(), ApiError> {
        let mut running = lock(&self.running);
        if running.contains_key(label) {
            return Err(ApiError::new(
                ErrorKind::AlreadyScanning,
                format!("location {label:?} is already being scanned"),
            ));
        }
        let handle = start()?;
        let events = handle.events().clone();
        let scan = Arc::new(RunningScan {
            handle: Mutex::new(Some(handle)),
            watchers: Mutex::new(Watchers {
                sinks: vec![sink],
                latest: None,
            }),
        });
        running.insert(label.to_owned(), Arc::clone(&scan));
        drop(running);

        let registry = Arc::clone(self);
        let label = label.to_owned();
        std::thread::spawn(move || {
            let started = Instant::now();
            let mut throttle = Throttle::new(PROGRESS_INTERVAL);
            let mut last_change: Option<Instant> = None;
            for event in &events {
                if matches!(event, ScanEvent::Progress(_))
                    && last_change.is_none_or(|at| at.elapsed() >= BROWSE_INTERVAL)
                {
                    last_change = Some(Instant::now());
                    registry.notify_browsers(&label, BrowseChange::Changed);
                }
                match event {
                    ScanEvent::Started { scan_id, .. } => scan.publish(ScanUpdate::Started {
                        label: label.clone(),
                        scan_id: scan_id.get(),
                    }),
                    ScanEvent::Progress(progress)
                        if throttle.admit(Instant::now(), progress.phase) =>
                    {
                        scan.publish(progress_update(&label, &progress, started.elapsed()));
                    }
                    ScanEvent::Finished(summary) => {
                        scan.publish(finished_update(&label, &summary));
                    }
                    _ => {}
                }
            }
            let handle = lock(&scan.handle).take();
            if let Some(Err(error)) = handle.map(ScanHandle::wait) {
                scan.publish(ScanUpdate::Failed {
                    label: label.clone(),
                    error: error.into(),
                });
            }
            {
                let mut running = lock(&registry.running);
                if running.get(&label).is_some_and(|s| Arc::ptr_eq(s, &scan)) {
                    running.remove(&label);
                }
            }
            registry.notify_browsers(&label, BrowseChange::Finished);
        });
        Ok(())
    }

    fn get(&self, label: &str) -> Result<Arc<RunningScan>, ApiError> {
        lock(&self.running).get(label).cloned().ok_or_else(|| {
            ApiError::new(
                ErrorKind::NotFound,
                format!("no scan of {label:?} is running"),
            )
        })
    }

    /// Notifies `sink` whenever a scan of `label` writes, from now on.
    pub(crate) fn subscribe_browse(&self, label: &str, sink: Box<dyn Sink<BrowseChange>>) {
        lock(&self.browsers)
            .entry(label.to_owned())
            .or_default()
            .push(sink);
    }

    fn notify_browsers(&self, label: &str, change: BrowseChange) {
        let mut browsers = lock(&self.browsers);
        if let Some(sinks) = browsers.get_mut(label) {
            sinks.retain(|sink| sink.send(change));
            if sinks.is_empty() {
                browsers.remove(label);
            }
        }
    }

    /// Adds `sink` to a running scan and sends it the scan's latest update at once.
    pub(crate) fn attach(
        &self,
        label: &str,
        sink: Box<dyn Sink<ScanUpdate>>,
    ) -> Result<(), ApiError> {
        let scan = self.get(label)?;
        let mut watchers = lock(&scan.watchers);
        let keep = watchers
            .latest
            .clone()
            .is_none_or(|latest| sink.send(latest));
        if keep {
            watchers.sinks.push(sink);
        }
        Ok(())
    }

    /// The latest update of every running scan.
    pub(crate) fn running(&self) -> Vec<ScanUpdate> {
        lock(&self.running)
            .values()
            .filter_map(|scan| lock(&scan.watchers).latest.clone())
            .collect()
    }

    pub(crate) fn is_running(&self, label: &str) -> bool {
        lock(&self.running).contains_key(label)
    }

    fn control(&self, label: &str, act: impl FnOnce(&ScanHandle)) -> Result<(), ApiError> {
        let scan = self.get(label)?;
        if let Some(handle) = lock(&scan.handle).as_ref() {
            act(handle);
        }
        Ok(())
    }

    pub(crate) fn pause(&self, label: &str) -> Result<(), ApiError> {
        self.control(label, ScanHandle::pause)
    }

    pub(crate) fn resume(&self, label: &str) -> Result<(), ApiError> {
        self.control(label, ScanHandle::resume)
    }

    pub(crate) fn cancel(&self, label: &str) -> Result<(), ApiError> {
        self.control(label, ScanHandle::cancel)
    }

    /// Cancels every scan and waits up to `timeout` for them to stop; scans keep what they
    /// committed.
    pub(crate) fn cancel_all_and_wait(&self, timeout: Duration) {
        let scans: Vec<_> = lock(&self.running).values().cloned().collect();
        for scan in scans {
            if let Some(handle) = lock(&scan.handle).as_ref() {
                handle.resume();
                handle.cancel();
            }
        }
        let deadline = Instant::now() + timeout;
        while !lock(&self.running).is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn progress_update(label: &str, progress: &ScanProgress, elapsed: Duration) -> ScanUpdate {
    let counts = progress.counts;
    ScanUpdate::Progress {
        label: label.to_owned(),
        phase: match progress.phase {
            ScanPhase::Walking => ScanPhaseView::Walking,
            ScanPhase::Hashing => ScanPhaseView::Hashing,
        },
        files: counts.files,
        dirs: counts.dirs,
        bytes: counts.bytes,
        hashed_files: counts.hashed_files,
        hashed_bytes: counts.hashed_bytes,
        to_hash_files: progress.to_hash_files,
        to_hash_bytes: progress.to_hash_bytes,
        // A rate is never negative or beyond u64 in practice; the cast saturates.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a byte rate rounded down to a whole number"
        )]
        bytes_per_sec: progress.bytes_per_sec.max(0.0) as u64,
        warnings: counts.warnings,
        elapsed_ms: millis(elapsed),
    }
}

fn finished_update(label: &str, summary: &ScanSummary) -> ScanUpdate {
    ScanUpdate::Finished {
        label: label.to_owned(),
        scan_id: summary.scan_id.get(),
        status: summary.status.into(),
        files: summary.counts.files,
        bytes: summary.counts.bytes,
        hashed_files: summary.counts.hashed_files,
        warnings: summary.counts.warnings,
        elapsed_ms: millis(summary.elapsed),
    }
}

impl AppState {
    /// Scans of every location, newest first.
    pub(crate) fn scans_recent(
        &self,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<RecentScanView>, ApiError> {
        Ok(Query::open(&self.catalog)?
            .recent_scans(&Page::at(offset, limit))?
            .items
            .iter()
            .map(|recent| RecentScanView {
                label: recent.label.clone(),
                scan: ScanBrief::from(&recent.scan),
            })
            .collect())
    }

    pub(crate) fn scans_recent_count(&self) -> Result<u64, ApiError> {
        Ok(Query::open(&self.catalog)?.recent_scan_count()?)
    }

    /// Scans of `label`, newest first.
    pub(crate) fn location_scans(
        &self,
        label: &str,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<ScanBrief>, ApiError> {
        Ok(Query::open(&self.catalog)?
            .scans(label, &Page::at(offset, limit))?
            .items
            .iter()
            .map(ScanBrief::from)
            .collect())
    }

    pub(crate) fn location_scan_count(&self, label: &str) -> Result<u64, ApiError> {
        Ok(Query::open(&self.catalog)?.scan_count(label)?)
    }

    /// Warnings of scan `scan_id`, in the order they were found.
    pub(crate) fn scan_warnings(
        &self,
        scan_id: i64,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<WarningView>, ApiError> {
        Ok(Query::open(&self.catalog)?
            .warnings(ScanId::from_i64(scan_id), &Page::at(offset, limit))?
            .items
            .into_iter()
            .map(|warning| WarningView {
                path: warning
                    .path
                    .map(|path| String::from_utf8_lossy(path.as_bytes()).into_owned()),
                kind: warning.kind.name().to_owned(),
                message: warning.message,
            })
            .collect())
    }

    pub(crate) fn scan_warning_count(&self, scan_id: i64) -> Result<u64, ApiError> {
        Ok(Query::open(&self.catalog)?.warning_count(ScanId::from_i64(scan_id))?)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::ops::ControlFlow;

    use symbolon_core::pathkey::RelPath;
    use symbolon_core::source::{
        LocalSource, ReadSeek, Source, SourceEntry, SourceError, WalkItem, WalkOutcome,
    };

    use super::*;
    use crate::sink::testing::{Collect, Gone};
    use crate::state::testing::state;

    /// Walks like the local source, `delay` per entry, so tests can act mid-scan.
    struct Slow(LocalSource, Duration);

    impl Source for Slow {
        fn walk(
            &self,
            visit: &mut dyn FnMut(WalkItem) -> ControlFlow<()>,
        ) -> Result<WalkOutcome, SourceError> {
            self.0.walk(&mut |item| {
                std::thread::sleep(self.1);
                visit(item)
            })
        }
        fn open(&self, path: &RelPath) -> io::Result<Box<dyn ReadSeek + Send>> {
            self.0.open(path)
        }
        fn stat(&self, path: &RelPath) -> io::Result<SourceEntry> {
            self.0.stat(path)
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        root: std::path::PathBuf,
        state: AppState,
        registry: Arc<ScanRegistry>,
    }

    fn fixture(files: usize) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("photos");
        fs::create_dir(&root).unwrap();
        for i in 0..files {
            fs::write(root.join(format!("f{i:04}.jpg")), format!("{i}")).unwrap();
        }
        let state = state(dir.path(), Vec::new());
        state
            .catalog
            .add_location(symbolon_core::catalog::NewLocation::local("Photos", &root))
            .unwrap();
        Fixture {
            _dir: dir,
            root,
            state,
            registry: Arc::new(ScanRegistry::default()),
        }
    }

    impl Fixture {
        fn start_slow(&self, sink: Box<dyn Sink<ScanUpdate>>) {
            let source = Arc::new(Slow(
                LocalSource::new(&self.root, &[]).unwrap(),
                Duration::from_millis(5),
            ));
            self.registry
                .start_with("Photos", sink, || {
                    Scanner::start_with_source(
                        &self.state.catalog,
                        "Photos",
                        source,
                        ScanOptions::default(),
                    )
                })
                .unwrap();
        }

        fn wait_idle(&self) {
            let deadline = Instant::now() + Duration::from_secs(20);
            while self.registry.is_running("Photos") {
                assert!(Instant::now() < deadline, "scan did not end");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn last_status(updates: &[ScanUpdate]) -> Option<ScanStatusView> {
        updates.iter().rev().find_map(|u| match u {
            ScanUpdate::Finished { status, .. } => Some(*status),
            _ => None,
        })
    }

    #[test]
    fn scan_reports_started_progress_and_finished() {
        let f = fixture(300);
        let sink = Collect::default();
        f.registry
            .start(&f.state.catalog, "Photos", Box::new(sink.clone()))
            .unwrap();
        f.wait_idle();
        let updates = sink.all();
        assert!(matches!(updates.first(), Some(ScanUpdate::Started { .. })));
        match updates.last() {
            Some(ScanUpdate::Finished { files, status, .. }) => {
                assert_eq!(*files, 300);
                assert_eq!(*status, ScanStatusView::Completed);
            }
            other => panic!("last update {other:?}"),
        }
    }

    #[test]
    fn progress_is_throttled() {
        let mut throttle = Throttle::new(Duration::from_millis(100));
        let t0 = Instant::now();
        let at = |ms| t0 + Duration::from_millis(ms);
        assert!(throttle.admit(at(0), ScanPhase::Walking));
        assert!(!throttle.admit(at(50), ScanPhase::Walking));
        assert!(throttle.admit(at(100), ScanPhase::Walking));
        assert!(!throttle.admit(at(150), ScanPhase::Walking));
        // A new phase always goes through.
        assert!(throttle.admit(at(160), ScanPhase::Hashing));
        assert!(!throttle.admit(at(200), ScanPhase::Hashing));
    }

    #[test]
    fn scan_survives_a_gone_sink() {
        let f = fixture(50);
        f.registry
            .start(&f.state.catalog, "Photos", Box::new(Gone))
            .unwrap();
        f.wait_idle();
        let recent = f.state.location_scans("Photos", 0, 10).unwrap();
        assert_eq!(recent[0].status, ScanStatusView::Completed);
        assert_eq!(recent[0].files, 50);
    }

    #[test]
    fn attach_gets_latest_then_continues() {
        let f = fixture(200);
        f.start_slow(Box::new(Gone));
        std::thread::sleep(Duration::from_millis(100));
        let late = Collect::default();
        f.registry.attach("Photos", Box::new(late.clone())).unwrap();
        assert!(!late.all().is_empty(), "the latest update arrives at once");
        f.wait_idle();
        assert_eq!(last_status(&late.all()), Some(ScanStatusView::Completed));
        assert_eq!(
            f.registry
                .attach("Photos", Box::new(Gone))
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
    }

    #[test]
    fn pause_resume_cancel() {
        let f = fixture(300);
        let sink = Collect::default();
        f.start_slow(Box::new(sink.clone()));
        std::thread::sleep(Duration::from_millis(50));
        f.registry.pause("Photos").unwrap();
        assert!(f.registry.is_running("Photos"));
        f.registry.resume("Photos").unwrap();
        f.registry.cancel("Photos").unwrap();
        f.wait_idle();
        assert_eq!(last_status(&sink.all()), Some(ScanStatusView::Cancelled));
    }

    #[test]
    fn second_start_is_already_scanning() {
        let f = fixture(200);
        f.start_slow(Box::new(Gone));
        let err = f
            .registry
            .start(&f.state.catalog, "Photos", Box::new(Gone))
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::AlreadyScanning);
        f.registry.cancel("Photos").unwrap();
        f.wait_idle();
    }

    #[test]
    fn running_lists_latest_updates() {
        let f = fixture(200);
        f.start_slow(Box::new(Gone));
        std::thread::sleep(Duration::from_millis(100));
        let running = f.registry.running();
        assert_eq!(running.len(), 1);
        f.registry.cancel_all_and_wait(Duration::from_secs(10));
        assert!(!f.registry.is_running("Photos"));
        assert_eq!(f.registry.running(), []);
    }

    #[test]
    fn remove_while_scanning_is_refused() {
        let f = fixture(200);
        let state = AppState {
            scans: Arc::clone(&f.registry),
            ..f.state.clone_for_test()
        };
        f.start_slow(Box::new(Gone));
        let err = state.location_remove("Photos").unwrap_err();
        assert_eq!(err.kind, ErrorKind::AlreadyScanning);
        assert_eq!(err.message, "Stop the scan first.");
        f.registry.cancel("Photos").unwrap();
        f.wait_idle();
        state.location_remove("Photos").unwrap();
    }

    #[test]
    fn history_and_warnings_page_by_offset() {
        let f = fixture(3);
        for _ in 0..3 {
            f.registry
                .start(&f.state.catalog, "Photos", Box::new(Gone))
                .unwrap();
            f.wait_idle();
        }
        assert_eq!(f.state.location_scan_count("Photos").unwrap(), 3);
        assert_eq!(f.state.location_scans("Photos", 1, 10).unwrap().len(), 2);
        assert_eq!(f.state.scans_recent_count().unwrap(), 3);
        let recent = f.state.scans_recent(0, 10).unwrap();
        assert_eq!(recent[0].label, "Photos");
        let id = recent[0].scan.scan_id;
        assert_eq!(f.state.scan_warning_count(id).unwrap(), 0);
        assert_eq!(f.state.scan_warnings(id, 0, 10).unwrap(), []);
    }
}
