//! Where streamed updates go: a Tauri channel in the app, a collecting sink in tests.

use tauri::ipc::{Channel, IpcResponse};

/// Receives updates; `false` means the receiver is gone and the sender should drop it.
pub(crate) trait Sink<T>: Send + Sync {
    fn send(&self, value: T) -> bool;
}

impl<T: IpcResponse + Send + Sync> Sink<T> for Channel<T> {
    fn send(&self, value: T) -> bool {
        Channel::send(self, value).is_ok()
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::sync::{Arc, Mutex, PoisonError};

    use super::Sink;

    /// Keeps everything sent to it; clones share the list.
    #[derive(Clone)]
    pub(crate) struct Collect<T>(Arc<Mutex<Vec<T>>>);

    impl<T> Default for Collect<T> {
        fn default() -> Self {
            Self(Arc::new(Mutex::new(Vec::new())))
        }
    }

    impl<T: Clone> Collect<T> {
        pub(crate) fn all(&self) -> Vec<T> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl<T: Send> Sink<T> for Collect<T> {
        fn send(&self, value: T) -> bool {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(value);
            true
        }
    }

    /// A receiver that is already gone.
    pub(crate) struct Gone;

    impl<T> Sink<T> for Gone {
        fn send(&self, _: T) -> bool {
            false
        }
    }
}
