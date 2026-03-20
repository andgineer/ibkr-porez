use ibkr_porez::config::{format_config_issues, validate_config};
use ibkr_porez::models::UserConfig;

fn valid_config() -> UserConfig {
    UserConfig {
        ibkr_token: "tok".into(),
        ibkr_query_id: "qid".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test Address 1".into(),
        city_code: "223".into(),
        phone: "0641234567".into(),
        email: "user@example.org".into(),
        data_dir: None,
        output_folder: None,
    }
}

#[test]
fn valid_config_returns_no_issues() {
    let issues = validate_config(&valid_config());
    assert!(issues.is_empty(), "expected no issues: {issues:?}");
}

#[test]
fn empty_ibkr_token() {
    let mut cfg = valid_config();
    cfg.ibkr_token = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "ibkr_token");
    assert_eq!(issues[0].message, "required");
}

#[test]
fn empty_ibkr_query_id() {
    let mut cfg = valid_config();
    cfg.ibkr_query_id = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "ibkr_query_id");
    assert_eq!(issues[0].message, "required");
}

#[test]
fn empty_personal_id() {
    let mut cfg = valid_config();
    cfg.personal_id = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "personal_id");
}

#[test]
fn empty_full_name() {
    let mut cfg = valid_config();
    cfg.full_name = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "full_name");
}

#[test]
fn empty_address() {
    let mut cfg = valid_config();
    cfg.address = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "address");
}

#[test]
fn empty_city_code() {
    let mut cfg = valid_config();
    cfg.city_code = String::new();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "city_code");
    assert_eq!(issues[0].label, "City Code");
}

#[test]
fn placeholder_phone() {
    let mut cfg = valid_config();
    cfg.phone = "0600000000".into();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "phone");
    assert_eq!(issues[0].message, "still the default placeholder");
}

#[test]
fn placeholder_email() {
    let mut cfg = valid_config();
    cfg.email = "email@example.com".into();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "email");
    assert_eq!(issues[0].message, "still the default placeholder");
}

#[test]
fn default_config_produces_multiple_issues() {
    let cfg = UserConfig::default();
    let issues = validate_config(&cfg);
    assert_eq!(issues.len(), 7);

    let fields: Vec<&str> = issues.iter().map(|i| i.field).collect();
    assert!(fields.contains(&"ibkr_token"));
    assert!(fields.contains(&"ibkr_query_id"));
    assert!(fields.contains(&"personal_id"));
    assert!(fields.contains(&"full_name"));
    assert!(fields.contains(&"address"));
    assert!(fields.contains(&"phone"));
    assert!(fields.contains(&"email"));
}

#[test]
fn format_config_issues_output() {
    let mut cfg = valid_config();
    cfg.ibkr_token = String::new();
    cfg.phone = "0600000000".into();
    let issues = validate_config(&cfg);
    let formatted = format_config_issues(&issues);
    assert!(formatted.contains("Configuration errors:"));
    assert!(formatted.contains("Flex Token: required"));
    assert!(formatted.contains("Phone: still the default placeholder"));
}

#[test]
fn is_field_helper() {
    let cfg = UserConfig::default();
    let issues = validate_config(&cfg);
    let token_issue = issues.iter().find(|i| i.is_field("ibkr_token"));
    assert!(token_issue.is_some());
    assert!(!issues.iter().any(|i| i.is_field("city_code")));
}
