use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, fmt};

#[must_use]
pub fn log_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ibkr-porez")
        .join("logs")
}

#[must_use]
pub fn init(verbose: bool) -> Option<WorkerGuard> {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    cleanup_old_logs(&dir);

    let appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("error.log")
        .build(&dir)
        .ok()?;

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .with_writer(non_blocking)
        .with_filter(tracing_subscriber::filter::LevelFilter::WARN);

    if verbose {
        let stderr_layer = fmt::layer()
            .with_target(false)
            .with_filter(tracing_subscriber::filter::LevelFilter::DEBUG);
        tracing_subscriber::registry()
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::registry().with(file_layer).init();
    }

    Some(guard)
}

fn cleanup_old_logs(dir: &Path) {
    let cutoff = chrono::Local::now().date_naive() - chrono::TimeDelta::days(90);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(date_str) = name.strip_prefix("error.log.") else {
            continue;
        };
        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            && date <= cutoff
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn cleanup_removes_old_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        let old = dir.join("error.log.2020-01-01");
        let recent = dir.join("error.log.2099-12-31");
        let unrelated = dir.join("other.log");
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&recent, "recent").unwrap();
        std::fs::write(&unrelated, "other").unwrap();

        cleanup_old_logs(dir);

        assert!(!old.exists(), "old log should be deleted");
        assert!(recent.exists(), "recent log should be kept");
        assert!(unrelated.exists(), "unrelated file should be kept");
    }

    #[test]
    fn cleanup_ignores_missing_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nonexistent = tmp.path().join("no_such_dir");
        cleanup_old_logs(&nonexistent); // must not panic
    }

    #[test]
    fn cleanup_boundary_exactly_90_days() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        let cutoff = chrono::Local::now().date_naive() - chrono::TimeDelta::days(90);
        let boundary = dir.join(format!("error.log.{cutoff}"));
        std::fs::write(&boundary, "x").unwrap();
        cleanup_old_logs(dir);
        assert!(
            !boundary.exists(),
            "file exactly at cutoff should be deleted"
        );
    }

    #[test]
    fn log_dir_returns_path_ending_in_logs() {
        let dir = log_dir();
        assert_eq!(dir.file_name().unwrap(), "logs");
    }

    #[test]
    fn parse_date_from_filename() {
        let date = NaiveDate::parse_from_str("2026-05-06", "%Y-%m-%d");
        assert!(date.is_ok());
    }
}
