use chrono::NaiveDate;
use ibkr_porez::models::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use tempfile::TempDir;

/// Cross-language compatibility test: write files with Rust, verify Python can read them.
///
/// Skips automatically when `uv` is not on PATH.
#[test]
fn test_python_reads_rust_written_files() {
    let Some(uv) = find_uv() else {
        eprintln!("skipping: `uv` not found on PATH");
        return;
    };

    let temp = TempDir::new().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::create_dir_all(data_dir.join("declarations")).unwrap();
    std::fs::create_dir_all(data_dir.join("flex-queries")).unwrap();

    write_test_config(&data_dir);
    write_test_transactions(&data_dir);
    write_test_rates(&data_dir);
    write_test_declarations(&data_dir);

    let test_script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_python_compat.py");

    let output = std::process::Command::new(&uv)
        .args(["run", test_script.to_str().unwrap(), data_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute `uv run`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Python compatibility test failed (exit code {:?}):\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code(),
    );

    assert!(
        stdout.contains("All Python compatibility checks passed"),
        "Unexpected Python output:\n{stdout}",
    );
}

fn find_uv() -> Option<String> {
    std::process::Command::new("uv")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| "uv".into())
}

fn write_test_config(data_dir: &std::path::Path) {
    let config = UserConfig {
        full_name: "Test User".into(),
        address: "Test Address".into(),
        data_dir: Some(data_dir.to_string_lossy().into_owned()),
        ..UserConfig::default()
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(data_dir.join("config.json"), json).unwrap();
}

fn write_test_transactions(data_dir: &std::path::Path) {
    let txns = vec![Transaction {
        transaction_id: "TEST-001".into(),
        date: NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
        r#type: TransactionType::Trade,
        symbol: "AAPL".into(),
        description: "Buy AAPL".into(),
        quantity: Decimal::from_str("10").unwrap(),
        price: Decimal::from_str("150.0").unwrap(),
        amount: Decimal::from_str("-1500.0").unwrap(),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: Some(Decimal::from_str("117.25").unwrap()),
        amount_rsd: Some(Decimal::from_str("-175875.0").unwrap()),
    }];
    let json = serde_json::to_string_pretty(&txns).unwrap();
    std::fs::write(data_dir.join("transactions.json"), json).unwrap();
}

fn write_test_rates(data_dir: &std::path::Path) {
    let mut rates = indexmap::IndexMap::new();
    rates.insert("2025-06-15_USD".to_string(), "117.25".to_string());
    let json = serde_json::to_string_pretty(&rates).unwrap();
    std::fs::write(data_dir.join("rates.json"), json).unwrap();
}

fn write_test_declarations(data_dir: &std::path::Path) {
    let decl = Declaration {
        declaration_id: "DECL-001".into(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        created_at: chrono::NaiveDateTime::parse_from_str(
            "2025-12-20T10:30:00",
            "%Y-%m-%dT%H:%M:%S",
        )
        .unwrap(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: None,
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    let decl_file = DeclarationsFile {
        declarations: vec![serde_json::to_value(&decl).unwrap()],
        last_declaration_date: Some("2025-12-31".into()),
    };
    let json = serde_json::to_string_pretty(&decl_file).unwrap();
    std::fs::write(data_dir.join("declarations.json"), json).unwrap();
}
