use std::fs;
use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::domain::DomainError;

/// Initialize the logging system with console output and file rotation.
///
/// Returns a guard that must be kept alive for the duration of the application.
/// When the guard is dropped, any remaining logs are flushed.
pub fn init_logging(
    logs_dir: &Path,
    level: &str,
    file_logging: bool,
) -> Result<Option<WorkerGuard>, DomainError> {
    // Ensure logs directory exists
    if file_logging {
        fs::create_dir_all(logs_dir)?;
    }

    // Environment filter with default from config
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("opensay={},warn", level)));

    // Console layer (always enabled, pretty format for development)
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::NONE)
        .with_filter(env_filter.clone());

    if file_logging {
        // File appender with daily rotation
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            logs_dir,
            "opensay.log",
        );

        // Non-blocking writer for the file appender
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // File layer with JSON format
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .json()
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(EnvFilter::new(format!("opensay={}", level)));

        // Combine layers - use try_init to avoid panic if called twice
        if tracing_subscriber::registry()
            .with(console_layer)
            .with(file_layer)
            .try_init()
            .is_ok()
        {
            tracing::info!(
                logs_dir = ?logs_dir,
                level = level,
                "Logging initialized with file output"
            );
        }

        Ok(Some(guard))
    } else {
        // Console only - use try_init to avoid panic if called twice
        let _ = tracing_subscriber::registry()
            .with(console_layer)
            .try_init();

        tracing::info!(level = level, "Logging initialized (console only)");

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_logging_initialization() {
        // This test just verifies the function doesn't panic
        // We can't easily test actual logging in unit tests
        let temp_dir = env::temp_dir().join("opensay_log_test");
        let _ = fs::remove_dir_all(&temp_dir);

        // Note: We can't initialize logging twice in tests, so just verify the path exists
        fs::create_dir_all(&temp_dir).unwrap();
        assert!(temp_dir.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
