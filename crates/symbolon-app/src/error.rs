//! The one error type every command returns, and the only place core errors are mapped to it.

use serde::Serialize;
use symbolon_core::catalog::CatalogError;
use symbolon_core::compare::CompareError;
use symbolon_core::scan::ScanError;
use symbolon_core::source::SourceError;
use symbolon_core::transport::android::AdbError;

/// What went wrong, for screens to react to; the message says it in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) enum ErrorKind {
    NotFound,
    AlreadyExists,
    AlreadyScanning,
    Moved,
    Ambiguous,
    OtherDevice,
    /// A folder or phone that cannot be reached right now.
    Unavailable,
    DeviceNotReady,
    AlgorithmMismatch,
    NotScannable,
    InvalidInput,
    Io,
    Internal,
}

/// A failed command: rejected promises in the frontend carry this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct ApiError {
    pub(crate) kind: ErrorKind,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }
}

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> Self {
        let kind = match &error {
            CatalogError::LabelTaken(_) => ErrorKind::AlreadyExists,
            CatalogError::LocationNotFound(_) => ErrorKind::NotFound,
            CatalogError::InvalidLabel(_) | CatalogError::InvalidRoot(_) => ErrorKind::InvalidInput,
            CatalogError::Io { .. } => ErrorKind::Io,
            CatalogError::Device(_) => ErrorKind::DeviceNotReady,
            _ => ErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<SourceError> for ApiError {
    fn from(error: SourceError) -> Self {
        let kind = match &error {
            SourceError::RootUnavailable { .. } => ErrorKind::Unavailable,
            SourceError::InvalidPattern { .. } => ErrorKind::InvalidInput,
            SourceError::Remote(_) => ErrorKind::Io,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<ScanError> for ApiError {
    fn from(error: ScanError) -> Self {
        let kind = match error {
            ScanError::Catalog(error) => return error.into(),
            ScanError::Source(error) => return error.into(),
            ScanError::AlreadyScanning { .. } => ErrorKind::AlreadyScanning,
            ScanError::Moved { .. } => ErrorKind::Moved,
            ScanError::Ambiguous { .. } => ErrorKind::Ambiguous,
            ScanError::OtherDevice { .. } => ErrorKind::OtherDevice,
            ScanError::NotScannable(_) | ScanError::Unsupported(_) => ErrorKind::NotScannable,
            ScanError::Lock(_) => ErrorKind::Io,
            ref _other => ErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<CompareError> for ApiError {
    fn from(error: CompareError) -> Self {
        let kind = match error {
            CompareError::Catalog(error) => return error.into(),
            CompareError::SideCount(_) | CompareError::InvalidSide(_) => ErrorKind::InvalidInput,
            CompareError::AlgorithmMismatch(_) => ErrorKind::AlgorithmMismatch,
            CompareError::Sqlite(_) => ErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<AdbError> for ApiError {
    fn from(error: AdbError) -> Self {
        let kind = match &error {
            AdbError::Io(_) => ErrorKind::Io,
            _ => ErrorKind::DeviceNotReady,
        };
        Self::new(kind, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use symbolon_core::hash::HashAlgo;

    use super::*;

    fn kind(error: impl Into<ApiError>) -> ErrorKind {
        error.into().kind
    }

    #[test]
    fn kinds_map_from_core_errors() {
        assert_eq!(
            kind(CatalogError::LabelTaken("a".into())),
            ErrorKind::AlreadyExists
        );
        assert_eq!(
            kind(CatalogError::LocationNotFound("a".into())),
            ErrorKind::NotFound
        );
        assert_eq!(
            kind(CatalogError::InvalidRoot(PathBuf::from("/x"))),
            ErrorKind::InvalidInput
        );
        assert_eq!(kind(CatalogError::Corrupt("x".into())), ErrorKind::Internal);
        assert_eq!(
            kind(ScanError::Catalog(CatalogError::LocationNotFound(
                "a".into()
            ))),
            ErrorKind::NotFound
        );
        assert_eq!(
            kind(ScanError::AlreadyScanning {
                label: "a".into(),
                since: None,
            }),
            ErrorKind::AlreadyScanning
        );
        assert_eq!(
            kind(ScanError::NotScannable("a".into())),
            ErrorKind::NotScannable
        );
        assert_eq!(
            kind(ScanError::Source(SourceError::RootUnavailable {
                root: "/gone".into(),
                source: std::io::Error::other("missing"),
            })),
            ErrorKind::Unavailable
        );
        assert_eq!(
            kind(ScanError::Moved {
                label: "a".into(),
                from: "/a".into(),
                to: "/b".into(),
            }),
            ErrorKind::Moved
        );
        assert_eq!(kind(CompareError::SideCount(1)), ErrorKind::InvalidInput);
        let mismatch = CompareError::AlgorithmMismatch(vec![
            ("a".into(), HashAlgo::Blake3),
            ("b".into(), HashAlgo::Sha256),
        ]);
        let error = ApiError::from(mismatch);
        assert_eq!(error.kind, ErrorKind::AlgorithmMismatch);
        assert!(error.message.contains('a') && error.message.contains('b'));
    }
}
