//! Event and log callback system.

use std::sync::{Mutex, OnceLock};

/// Log level for debug callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

type EventCallback = Box<dyn Fn(&str, &str) + Send + Sync + 'static>;
type LogCallback = Box<dyn Fn(LogLevel, &str) + Send + Sync + 'static>;

fn event_callback() -> &'static Mutex<Option<EventCallback>> {
    static CALLBACK: OnceLock<Mutex<Option<EventCallback>>> = OnceLock::new();
    CALLBACK.get_or_init(|| Mutex::new(None))
}

fn log_callback() -> &'static Mutex<Option<LogCallback>> {
    static CALLBACK: OnceLock<Mutex<Option<LogCallback>>> = OnceLock::new();
    CALLBACK.get_or_init(|| Mutex::new(None))
}

/// Set the global event callback.
pub fn set_event_callback<F>(callback: F)
where
    F: Fn(&str, &str) + Send + Sync + 'static,
{
    let mut guard = event_callback().lock().expect("event callback lock");
    *guard = Some(Box::new(callback));
}

/// Emit an event to the registered callback.
pub fn emit_event(name: &str, data: &str) {
    if let Ok(guard) = event_callback().lock() {
        if let Some(callback) = guard.as_ref() {
            callback(name, data);
        }
    }
}

/// Set the global log callback.
pub fn set_log_callback<F>(callback: F)
where
    F: Fn(LogLevel, &str) + Send + Sync + 'static,
{
    let mut guard = log_callback().lock().expect("log callback lock");
    *guard = Some(Box::new(callback));
}

/// Emit a log event.
pub fn emit_log(level: LogLevel, message: &str) {
    if let Ok(guard) = log_callback().lock() {
        if let Some(callback) = guard.as_ref() {
            callback(level, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_callback() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        set_event_callback(move |name, _data| {
            assert_eq!(name, "test");
            called_clone.store(true, Ordering::SeqCst);
        });
        emit_event("test", "{}");
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_log_callback() {
        set_log_callback(|level, msg| {
            assert_eq!(level, LogLevel::Info);
            assert_eq!(msg, "hello");
        });
        emit_log(LogLevel::Info, "hello");
    }
}
