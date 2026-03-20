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
