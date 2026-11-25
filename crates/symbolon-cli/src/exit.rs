//! Exit codes (ADR-0011).

use std::fmt;
use std::process::ExitCode;

use symbolon_core::catalog::TransferError;
use symbolon_core::scan::ScanError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exit {
    Success,
    /// `diff` found differences.
    Differences,
    Error,
    /// Refused until the user chooses, for example with a flag.
    Refused,
    /// Stopped by Ctrl-C after cleaning up.
    Interrupted,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        Self::from(match exit {
            Exit::Success => 0,
            Exit::Differences => 1,
            Exit::Error => 3,
            Exit::Refused => 4,
            Exit::Interrupted => 130,
        })
    }
}

impl Exit {
    pub(crate) fn for_error(error: &anyhow::Error) -> Self {
        let refused = error.chain().any(|cause| {
            cause.is::<Refused>()
                || matches!(
                    cause.downcast_ref::<ScanError>(),
                    Some(
                        ScanError::AlreadyScanning { .. }
                            | ScanError::Moved { .. }
                            | ScanError::Ambiguous { .. }
                            | ScanError::OtherDevice { .. }
                    )
                )
                || matches!(
                    cause.downcast_ref::<TransferError>(),
                    Some(TransferError::Busy(_) | TransferError::Conflict(_))
                )
        });
        if refused { Self::Refused } else { Self::Error }
    }
}

/// An error the user resolves by choosing; the message names the choices.
#[derive(Debug)]
pub(crate) struct Refused(pub(crate) String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}
