//! A panic hook that emits an error-level `tracing` event when a panic occurs.
//!
//! Check out [`panic_hook`]'s documentation for more information.
use std::panic::PanicInfo;

/// A panic hook that emits an error-level `tracing` event when a panic occurs.
///
/// The default panic hook prints the panic information to stderr, which might or
/// might not be picked up by your telemetry system.
///
/// This hook, instead, makes sure that panic information goes through the `tracing`
/// pipeline you've configured.
///
/// # Usage
///
/// ```rust
/// use tracing_panic::panic_hook;
///
/// # #[allow(clippy::needless_doctest_main)]
/// fn main() {
///     // Initialize your `tracing` subscriber however you like.
///     // [...]
///     // Then set the panic hook.
///     // This should be done only once, at the beginning of your program.
///     std::panic::set_hook(Box::new(panic_hook));
/// }
/// ```
///
/// # Backtrace
///
/// The hook currently doesn't try to capture a backtrace.
pub fn panic_hook(panic_info: &PanicInfo) {
    let payload = panic_info.payload();

    #[allow(clippy::manual_map)]
    let payload = if let Some(s) = payload.downcast_ref::<&str>() {
        Some(&**s)
    } else if let Some(s) = payload.downcast_ref::<String>() {
        Some(s.as_str())
    } else {
        None
    };

    let location = panic_info.location().map(|l| l.to_string());

    tracing::error!(
        panic.payload = payload,
        panic.location = location,
        "A panic occurred",
    );
}

#[cfg(test)]
mod tests {
    use tracing::subscriber::DefaultGuard;

    use super::panic_hook;
    use std::io;
    use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

    #[test]
    fn test_static_panic_message() {
        let buffer = Arc::new(Mutex::new(vec![]));
        let _guard = init_subscriber(buffer.clone());
        let _ = std::panic::catch_unwind(|| {
            std::panic::set_hook(Box::new(panic_hook));
            panic!("This is a static panic message");
        });

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("This is a static panic message"));
    }

    #[test]
    fn test_interpolated_panic_message() {
        let buffer = Arc::new(Mutex::new(vec![]));
        let _guard = init_subscriber(buffer.clone());

        let _ = std::panic::catch_unwind(|| {
            std::panic::set_hook(Box::new(panic_hook));
            panic!("This is an {} panic message", "interpolated");
        });

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("This is an interpolated panic message"));
    }

    fn init_subscriber(buffer: Arc<Mutex<Vec<u8>>>) -> DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || MockWriter::new(buffer.clone()))
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    /// Use a vector of bytes behind a Arc<Mutex> as writer in order to inspect the tracing output
    /// for testing purposes.
    pub struct MockWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl MockWriter {
        pub fn new(buf: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { buf }
        }

        pub fn map_error<Guard>(err: TryLockError<Guard>) -> io::Error {
            match err {
                TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
                TryLockError::Poisoned(_) => io::Error::from(io::ErrorKind::Other),
            }
        }

        pub fn buf(&self) -> io::Result<MutexGuard<'_, Vec<u8>>> {
            self.buf.try_lock().map_err(Self::map_error)
        }
    }

    impl io::Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buf()?.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.buf()?.flush()
        }
    }
}
