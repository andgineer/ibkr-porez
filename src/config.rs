use std::path::PathBuf;

use anyhow::Result;

use crate::models::UserConfig;

const APP_NAME: &str = "ibkr-porez";
const DATA_SUBDIR: &str = "ibkr-porez-data";
const CONFIG_FILENAME: &str = "config.json";

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

pub const FLEX_URL_ENV: &str = "IBKR_POREZ_FLEX_URL";
pub const NBS_URL_ENV: &str = "IBKR_POREZ_NBS_URL";
pub const HOLIDAYS_URL_ENV: &str = "IBKR_POREZ_HOLIDAYS_URL";

/// Redirects an outbound service to another host. Tests that spawn the real
/// binary set these so a run can never reach the live service — most sharply
/// for IBKR, which counts failed attempts and locks out the real user once
/// they pile up.
#[must_use]
pub fn base_url_override(env_key: &str) -> Option<String> {
    std::env::var(env_key).ok().filter(|url| !url.is_empty())
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

#[must_use]
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("IBKR_POREZ_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

#[must_use]
pub fn config_file_path() -> PathBuf {
    config_dir().join(CONFIG_FILENAME)
}

#[must_use]
pub fn get_default_data_dir_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
        .join(DATA_SUBDIR)
}

#[must_use]
pub fn get_effective_data_dir_path(config: &UserConfig) -> PathBuf {
    match &config.data_dir {
        Some(dir) if !dir.is_empty() => {
            let p = expand_tilde(dir);
            std::fs::canonicalize(&p).unwrap_or(p)
        }
        _ => {
            let p = get_default_data_dir_path();
            std::fs::canonicalize(&p).unwrap_or(p)
        }
    }
}

#[must_use]
pub fn get_default_output_dir_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Downloads")
}

#[must_use]
pub fn get_effective_output_dir_path(config: &UserConfig) -> PathBuf {
    match &config.output_folder {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => get_default_output_dir_path(),
    }
}

#[must_use]
pub fn get_data_dir_change_warning(old: &UserConfig, new: &UserConfig) -> Option<String> {
    let old_path = get_effective_data_dir_path(old);
    let new_path = get_effective_data_dir_path(new);
    if old_path == new_path {
        return None;
    }
    Some(format!(
        "Data directory changed. Move existing database files manually from {} to {}.",
        old_path.display(),
        new_path.display()
    ))
}

// ---------------------------------------------------------------------------
// Config load / save
// ---------------------------------------------------------------------------

#[must_use]
pub fn load_config_from(path: &std::path::Path) -> UserConfig {
    if !path.exists() {
        return UserConfig::default();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => UserConfig::default(),
    }
}

#[must_use]
pub fn load_config() -> UserConfig {
    load_config_from(&config_file_path())
}

pub fn save_config(config: &UserConfig) -> Result<()> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ConfigIssue {
    pub field: &'static str,
    pub label: &'static str,
    pub message: &'static str,
}

impl ConfigIssue {
    #[must_use]
    pub fn is_field(&self, name: &str) -> bool {
        self.field == name
    }
}

#[must_use]
pub fn validate_config(config: &UserConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();

    let required: &[(&str, &str, &str)] = &[
        ("ibkr_token", "Flex Token", config.ibkr_token.as_str()),
        (
            "ibkr_query_id",
            "Flex Query ID",
            config.ibkr_query_id.as_str(),
        ),
        (
            "personal_id",
            "Personal ID (JMBG)",
            config.personal_id.as_str(),
        ),
        ("full_name", "Full Name", config.full_name.as_str()),
        ("address", "Address", config.address.as_str()),
        ("city_code", "City Code", config.city_code.as_str()),
    ];

    for &(field, label, value) in required {
        if value.is_empty() {
            issues.push(ConfigIssue {
                field,
                label,
                message: "required",
            });
        }
    }

    if config.phone == "0600000000" {
        issues.push(ConfigIssue {
            field: "phone",
            label: "Phone",
            message: "still the default placeholder",
        });
    }
    if config.email == "email@example.com" {
        issues.push(ConfigIssue {
            field: "email",
            label: "Email",
            message: "still the default placeholder",
        });
    }

    issues
}

#[must_use]
pub fn format_config_issues(issues: &[ConfigIssue]) -> String {
    let mut lines = vec!["Configuration errors:".to_string()];
    for issue in issues {
        lines.push(format!("  - {}: {}", issue.label, issue.message));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_config() -> UserConfig {
        UserConfig {
            ibkr_token: "token".into(),
            ibkr_query_id: "query".into(),
            personal_id: "1234567890123".into(),
            full_name: "Test User".into(),
            address: "Test Address 1".into(),
            city_code: "11000".into(),
            phone: "0611234567".into(),
            email: "test@example.org".into(),
            ..UserConfig::default()
        }
    }

    #[test]
    fn validate_config_passes_full_config() {
        assert!(validate_config(&full_config()).is_empty());
    }

    #[test]
    fn validate_config_catches_missing_token() {
        let mut cfg = full_config();
        cfg.ibkr_token = String::new();
        let issues = validate_config(&cfg);
        assert!(issues.iter().any(|i| i.field == "ibkr_token"));
    }

    #[test]
    fn validate_config_catches_default_phone() {
        let mut cfg = full_config();
        cfg.phone = "0600000000".into();
        let issues = validate_config(&cfg);
        assert!(issues.iter().any(|i| i.field == "phone"));
    }

    #[test]
    fn validate_config_catches_default_email() {
        let mut cfg = full_config();
        cfg.email = "email@example.com".into();
        let issues = validate_config(&cfg);
        assert!(issues.iter().any(|i| i.field == "email"));
    }

    #[test]
    fn validate_config_all_empty() {
        let issues = validate_config(&UserConfig::default());
        assert!(issues.len() >= 6, "at least 6 required fields");
    }

    #[test]
    fn format_config_issues_contains_labels() {
        let issues = validate_config(&UserConfig::default());
        let text = format_config_issues(&issues);
        assert!(text.contains("Configuration errors:"));
        assert!(text.contains("Flex Token"));
        assert!(text.contains("Personal ID"));
    }

    #[test]
    fn config_issue_is_field() {
        let issue = ConfigIssue {
            field: "phone",
            label: "Phone",
            message: "bad",
        };
        assert!(issue.is_field("phone"));
        assert!(!issue.is_field("email"));
    }

    #[test]
    fn expand_tilde_with_home() {
        let result = expand_tilde("~/Documents/test");
        assert!(!result.starts_with("~"));
        assert!(result.to_str().unwrap().contains("Documents/test"));
    }

    #[test]
    fn expand_tilde_without_tilde() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result, std::path::PathBuf::from("/absolute/path"));
    }

    #[test]
    fn effective_data_dir_custom() {
        let tmp = tempfile::TempDir::new().unwrap();
        let canonical = std::fs::canonicalize(tmp.path()).unwrap();
        let cfg = UserConfig {
            data_dir: Some(tmp.path().display().to_string()),
            ..UserConfig::default()
        };
        let path = get_effective_data_dir_path(&cfg);
        assert_eq!(path, canonical);
    }

    #[test]
    fn effective_data_dir_default() {
        let cfg = UserConfig::default();
        let path = get_effective_data_dir_path(&cfg);
        assert!(
            path.to_str().unwrap().contains("ibkr-porez"),
            "default data dir should contain app name"
        );
    }

    #[test]
    fn effective_output_dir_custom() {
        let cfg = UserConfig {
            output_folder: Some("/custom/output".into()),
            ..UserConfig::default()
        };
        let path = get_effective_output_dir_path(&cfg);
        assert_eq!(path, std::path::PathBuf::from("/custom/output"));
    }

    #[test]
    fn effective_output_dir_default() {
        let cfg = UserConfig::default();
        let path = get_effective_output_dir_path(&cfg);
        assert!(
            path.to_str().unwrap().contains("Downloads"),
            "default output dir should be Downloads"
        );
    }

    #[test]
    fn data_dir_change_warning_same_path() {
        let cfg = full_config();
        assert!(get_data_dir_change_warning(&cfg, &cfg).is_none());
    }

    #[test]
    fn data_dir_change_warning_different_paths() {
        let tmp1 = tempfile::TempDir::new().unwrap();
        let tmp2 = tempfile::TempDir::new().unwrap();
        let old = UserConfig {
            data_dir: Some(tmp1.path().display().to_string()),
            ..UserConfig::default()
        };
        let new = UserConfig {
            data_dir: Some(tmp2.path().display().to_string()),
            ..UserConfig::default()
        };
        let warning = get_data_dir_change_warning(&old, &new);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("Move existing"));
    }

    #[test]
    fn load_config_from_missing_file_returns_default() {
        let cfg = load_config_from(std::path::Path::new("/nonexistent/config.json"));
        assert_eq!(cfg, UserConfig::default());
    }

    #[test]
    fn load_config_from_valid_json() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"ibkr_token":"tok","ibkr_query_id":"qid"}"#).unwrap();
        let cfg = load_config_from(tmp.path());
        assert_eq!(cfg.ibkr_token, "tok");
        assert_eq!(cfg.ibkr_query_id, "qid");
    }

    #[test]
    fn load_config_from_invalid_json_returns_default() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "not json").unwrap();
        let cfg = load_config_from(tmp.path());
        assert_eq!(cfg, UserConfig::default());
    }
}
