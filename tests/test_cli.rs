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
    let output = cmd()
        .env("IBKR_POREZ_DRY_RUN", "1")
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("GUI") || stderr.contains("GUI") || stderr.contains("GUI binary not found"),
        "expected GUI dispatch attempt, got stdout={stdout:?} stderr={stderr:?}"
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
