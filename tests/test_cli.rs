use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("ibkr-porez").unwrap()
}

#[test]
fn version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("ibkr-porez"));
}

#[test]
fn help_flag() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Serbian tax reporting"));
}

#[test]
fn help_all_subcommands() {
    let subcommands = [
        "config",
        "fetch",
        "import",
        "sync",
        "report",
        "list",
        "show",
        "stat",
        "submit",
        "pay",
        "assess",
        "export",
        "export-flex",
        "revert",
        "attach",
    ];

    for sub in subcommands {
        cmd()
            .args([sub, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn report_half_short_flag_does_not_conflict_with_help() {
    cmd()
        .args(["report", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--half"));
}

#[test]
fn list_ids_only_flag() {
    cmd()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--ids-only").and(predicate::str::contains("-1")));
}

#[test]
fn export_flex_stdout_flag() {
    cmd()
        .args(["export-flex", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn no_subcommand_dispatches_to_gui() {
    // `launch_gui()` looks for a sibling `ibkr-porez-gui` executable next to the
    // current CLI binary. This test copies the CLI into a temp dir, places a
    // fake GUI executable beside it, and verifies the fake GUI was launched by
    // checking that it created a marker file.
    let tmp = tempfile::TempDir::new().unwrap();
    let marker = tmp.path().join("gui-launched.marker");

    let cli_src = Command::cargo_bin("ibkr-porez")
        .unwrap()
        .get_program()
        .to_owned();
    let cli_name = format!("ibkr-porez{}", std::env::consts::EXE_SUFFIX);
    let cli_copy = tmp.path().join(&cli_name);
    std::fs::copy(&cli_src, &cli_copy).unwrap();

    let gui_name = format!("ibkr-porez-gui{}", std::env::consts::EXE_SUFFIX);
    let fake_gui = tmp.path().join(&gui_name);
    let fake_src = tmp.path().join("fake_gui.rs");
    std::fs::write(
        &fake_src,
        r#"fn main() {
            if let Ok(p) = std::env::var("_IBKR_TEST_MARKER") {
                let _ = std::fs::write(p, "ok");
            }
        }"#,
    )
    .unwrap();

    let rc = std::process::Command::new("rustc")
        .args([fake_src.to_str().unwrap(), "-o", fake_gui.to_str().unwrap()])
        .status()
        .expect("rustc must be available");
    assert!(rc.success(), "failed to compile fake GUI binary");

    assert!(!marker.exists(), "precondition: marker must not exist yet");

    let output = std::process::Command::new(&cli_copy)
        .env("_IBKR_TEST_MARKER", &marker)
        .output()
        .expect("failed to run cli copy");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Starting GUI"),
        "CLI should attempt to launch GUI, stderr: {stderr:?}"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !marker.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(375));
    }
    assert!(
        marker.exists(),
        "fake GUI binary should have been spawned and created marker file"
    );
}

#[test]
fn verbose_flag_accepted_globally() {
    cmd().args(["-v", "--help"]).assert().success();
}

#[test]
fn import_empty_stdin_imports_zero() {
    cmd()
        .args(["import"])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("No valid transactions found"));
}

#[test]
fn import_nonexistent_file_falls_back_to_stdin() {
    cmd()
        .args(["import", "/nonexistent/file.csv"])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("No valid transactions found"));
}

#[test]
fn show_nonexistent_declaration_is_error() {
    cmd()
        .args(["show", "nonexistent-id-12345"])
        .assert()
        .success()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn submit_nonexistent_declaration_is_error() {
    let assert = cmd()
        .args(["submit", "nonexistent-id-12345"])
        .assert()
        .failure();
    assert.stderr(predicate::str::contains("not found"));
}

#[test]
fn revert_nonexistent_declaration_is_error() {
    let assert = cmd()
        .args(["revert", "nonexistent-id-12345"])
        .assert()
        .failure();
    assert.stderr(predicate::str::contains("not found"));
}

#[test]
fn pay_nonexistent_declaration_is_error() {
    let assert = cmd()
        .args(["pay", "nonexistent-id-12345"])
        .assert()
        .failure();
    assert.stderr(predicate::str::contains("not found"));
}

#[test]
fn assess_requires_tax_due() {
    cmd()
        .args(["assess", "some-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tax-due"));
}

#[test]
fn export_flex_requires_date() {
    cmd()
        .args(["export-flex"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("<DATE>").or(predicate::str::contains("required")));
}

// ---- Multi-ID and pipeline tests ----

#[test]
fn submit_no_args_empty_stdin_fails() {
    cmd()
        .arg("submit")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no declaration IDs provided"));
}

#[test]
fn pay_no_args_empty_stdin_fails() {
    cmd()
        .arg("pay")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no declaration IDs provided"));
}

#[test]
fn revert_no_args_empty_stdin_fails() {
    cmd()
        .arg("revert")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no declaration IDs provided"));
}

#[test]
fn submit_multiple_nonexistent_ids() {
    cmd()
        .args(["submit", "x", "y", "z"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("x")
                .and(predicate::str::contains("y"))
                .and(predicate::str::contains("z")),
        );
}

#[test]
fn pay_tax_with_multiple_ids_rejected() {
    cmd()
        .args(["pay", "--tax", "100", "x", "y"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--tax can only be used with a single declaration ID",
        ));
}

#[test]
fn submit_reads_ids_from_stdin() {
    cmd()
        .arg("submit")
        .write_stdin("id1\nid2\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("id1").and(predicate::str::contains("id2")));
}

#[test]
fn pay_reads_ids_from_stdin() {
    cmd()
        .arg("pay")
        .write_stdin("id1\nid2\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("id1").and(predicate::str::contains("id2")));
}

#[test]
fn revert_reads_ids_from_stdin() {
    cmd()
        .arg("revert")
        .write_stdin("id1\nid2\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("id1").and(predicate::str::contains("id2")));
}
