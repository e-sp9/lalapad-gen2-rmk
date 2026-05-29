use crate::iqs9151::{TrackpadButton, TrackpadSide, VirtualKeyPosition, trackpad_button_position};
use std::collections::BTreeMap;
use std::process::Command;

const KEYBOARD_TOML: &str = include_str!("../keyboard.toml");
const AUTO_TAG_WORKFLOW_YAML: &str = include_str!("../.github/workflows/auto-tag.yml");
const FIRMWARE_WORKFLOW_YAML: &str = include_str!("../.github/workflows/firmware.yml");
const HARDWARE_VALIDATION_MANIFEST_TOML: &str =
    include_str!("../tools/hardware_validation_manifest.toml");
const HARDWARE_VALIDATION_EVIDENCE_EXAMPLE_TOML: &str =
    include_str!("../tools/hardware_validation_evidence.example.toml");
const MAKEFILE_TOML: &str = include_str!("../Makefile.toml");
const PORTING_COVERAGE_MANIFEST_TOML: &str =
    include_str!("../tools/porting_coverage_manifest.toml");
const PULL_REQUEST_TEMPLATE_MD: &str = include_str!("../.github/PULL_REQUEST_TEMPLATE.md");
const VIAL_JSON: &str = include_str!("../vial.json");

fn keyboard_toml() -> toml::Value {
    toml::from_str(KEYBOARD_TOML).unwrap()
}

fn porting_coverage_manifest_toml() -> toml::Value {
    toml::from_str(PORTING_COVERAGE_MANIFEST_TOML).unwrap()
}

fn hardware_validation_manifest_toml() -> toml::Value {
    toml::from_str(HARDWARE_VALIDATION_MANIFEST_TOML).unwrap()
}

fn vial_json() -> serde_json::Value {
    serde_json::from_str(VIAL_JSON).unwrap()
}

fn run_porting_coverage(args: &[&str]) -> std::process::Output {
    Command::new("python3")
        .arg("tools/porting_coverage.py")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn run_hardware_validation(args: &[&str]) -> std::process::Output {
    Command::new("python3")
        .arg("tools/hardware_validation.py")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn run_migration_status(args: &[&str]) -> std::process::Output {
    Command::new("python3")
        .arg("tools/migration_status.py")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn write_temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("lalapad-{name}-{}.toml", std::process::id()));
    std::fs::write(&path, contents).unwrap();
    path
}

fn run_python(script: &str) -> std::process::Output {
    Command::new("python3")
        .arg("-c")
        .arg(script)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn default_zmk_config_dir() -> Option<std::path::PathBuf> {
    for relative in [
        "../zmk-config-LalaPadGen2/config",
        "zmk-config-LalaPadGen2/config",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        if path.join("lalapadgen2.keymap").exists() {
            return Some(path);
        }
    }
    None
}

fn manifest_status_counts(manifest: &toml::Value) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for section in ["iqs9151_register_porting", "iqs9151_bit_porting"] {
        for entry in manifest[section].as_array().unwrap() {
            let status = entry["status"].as_str().unwrap().to_string();
            *counts.entry(status).or_insert(0) += 1;
        }
    }
    counts
}

#[test]
fn porting_coverage_manifest_is_satisfied() {
    let output = run_porting_coverage(&[]);

    assert!(
        output.status.success(),
        "porting coverage failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn porting_coverage_complete_gate_accepts_explicit_status_completion() {
    let output = run_porting_coverage(&["--require-porting-complete"]);

    assert!(
        output.status.success(),
        "complete porting gate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Porting coverage by kind:"));
    assert!(stdout.contains("- scenario:"));
    assert!(stdout.contains("- zmk_source_cell:"));
    assert!(stdout.contains("Porting status: 69/69 = 100.00% implemented"));
}

#[test]
fn firmware_ci_runs_complete_porting_gate_before_builds() {
    let gate_command = "python3 tools/porting_coverage.py";
    let zmk_required_flag = "--require-zmk-source";
    let complete_required_flag = "--require-porting-complete";
    let host_tests = "cargo test --lib --target x86_64-unknown-linux-gnu";
    let release_build = "cargo make build";

    for required in [gate_command, zmk_required_flag, complete_required_flag] {
        assert!(
            FIRMWARE_WORKFLOW_YAML.contains(required),
            "firmware CI is missing complete porting gate component {required:?}"
        );
    }

    let gate_index = FIRMWARE_WORKFLOW_YAML.find(gate_command).unwrap();
    let zmk_required_index = FIRMWARE_WORKFLOW_YAML.find(zmk_required_flag).unwrap();
    let complete_required_index = FIRMWARE_WORKFLOW_YAML.find(complete_required_flag).unwrap();
    let host_tests_index = FIRMWARE_WORKFLOW_YAML.find(host_tests).unwrap();
    let release_build_index = FIRMWARE_WORKFLOW_YAML.find(release_build).unwrap();

    assert!(gate_index < zmk_required_index);
    assert!(gate_index < complete_required_index);
    assert!(
        complete_required_index < host_tests_index,
        "complete porting gate must run before host parity tests"
    );
    assert!(
        complete_required_index < release_build_index,
        "complete porting gate must run before release binaries are built"
    );
}

#[test]
fn hardware_validation_manifest_is_classified_but_not_release_blocking() {
    let output = run_hardware_validation(&["--json", "--require-classified"]);

    assert!(
        output.status.success(),
        "hardware validation tracker failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let manifest = hardware_validation_manifest_toml();
    let checks = manifest["checks"].as_array().unwrap();
    let expected_total = checks.len() as i64;
    assert_eq!(parsed["total"].as_i64(), Some(expected_total));
    assert_eq!(parsed["classified"].as_bool(), Some(true));
    assert_eq!(parsed["errors"].as_array().unwrap().len(), 0);
    assert!(
        parsed["by_status"]["requires_hardware"].as_i64().unwrap() > 0,
        "real-hardware validation gaps should remain visible separately from source porting"
    );
    assert_eq!(parsed["by_area"]["trackpad"]["total"].as_i64(), Some(7));
    assert_eq!(parsed["by_area"]["trackpad"]["validated"].as_i64(), Some(0));
    assert_eq!(
        parsed["by_area"]["trackpad"]["by_status"]["requires_hardware"].as_i64(),
        Some(7)
    );
    assert_eq!(parsed["by_side"]["right"]["total"].as_i64(), Some(5));
    assert_eq!(parsed["by_side"]["left"]["total"].as_i64(), Some(3));
    assert_eq!(parsed["by_side"]["both"]["total"].as_i64(), Some(4));
    assert!(
        FIRMWARE_WORKFLOW_YAML
            .contains("python3 tools/hardware_validation.py --require-classified"),
        "firmware CI should keep the real-hardware validation tracker classified"
    );
    assert!(
        FIRMWARE_WORKFLOW_YAML.contains(
            "python3 tools/hardware_validation.py --markdown >> \"$GITHUB_STEP_SUMMARY\""
        ),
        "firmware CI should publish a hardware validation summary table"
    );
    assert!(
        !FIRMWARE_WORKFLOW_YAML.contains("--require-validated"),
        "release CI must not claim real-hardware validation without physical evidence"
    );

    let by_id: BTreeMap<_, _> = checks
        .iter()
        .map(|check| (check["id"].as_str().unwrap(), check))
        .collect();
    for (required_id, area, side) in [
        ("iqs9151_right_i2c_identity", "trackpad", "right"),
        ("iqs9151_left_i2c_identity", "trackpad", "left"),
        ("iqs9151_right_rdy_signal", "trackpad", "right"),
        ("iqs9151_left_rdy_signal", "trackpad", "left"),
        ("right_trackpad_cursor_tap_scroll", "trackpad", "right"),
        ("left_trackpad_split_cursor_tap_scroll", "trackpad", "left"),
        ("trackpad_drag_cross_side", "trackpad", "both"),
        ("ble_split_pairing_reconnect", "ble_split", "both"),
        ("vial_thumb_layer_taps", "vial", "both"),
        (
            "rgb_battery_connection_layer_indicators",
            "status_led",
            "right",
        ),
        ("charge_indicator_pins", "battery", "right"),
        ("storage_reset_and_reflash", "storage", "both"),
    ] {
        let check = by_id
            .get(required_id)
            .unwrap_or_else(|| panic!("hardware validation check {required_id} is missing"));
        assert_eq!(check["area"].as_str(), Some(area));
        assert_eq!(check["side"].as_str(), Some(side));
    }
}

#[test]
fn migration_status_combines_software_and_hardware_progress() {
    let output = run_migration_status(&[
        "--json",
        "--require-software-complete",
        "--require-hardware-classified",
    ]);

    assert!(
        output.status.success(),
        "migration status failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["software"]["complete"].as_bool(), Some(true));
    assert_eq!(
        parsed["software"]["passed"].as_i64(),
        parsed["software"]["total"].as_i64()
    );
    assert_eq!(
        parsed["software"]["implementation"]["implemented"].as_i64(),
        parsed["software"]["implementation"]["total"].as_i64()
    );
    assert_eq!(
        parsed["software"]["by_kind"]["scenario"]["rate"].as_f64(),
        Some(1.0)
    );
    assert_eq!(parsed["hardware"]["classified"].as_bool(), Some(true));
    assert_eq!(parsed["hardware"]["validated"].as_i64(), Some(0));
    assert_eq!(parsed["hardware"]["total"].as_i64(), Some(12));
    assert_eq!(
        parsed["ready_for_release_without_hardware"].as_bool(),
        Some(true)
    );
    assert_eq!(parsed["fully_validated"].as_bool(), Some(false));

    let markdown = run_migration_status(&["--markdown"]);
    assert!(
        markdown.status.success(),
        "migration status markdown failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&markdown.stdout),
        String::from_utf8_lossy(&markdown.stderr)
    );
    let stdout = String::from_utf8_lossy(&markdown.stdout);
    assert!(stdout.contains("## RMK Migration Status"));
    assert!(stdout.contains("| Software coverage | 2351 | 2351 | 100.00% |"));
    assert!(stdout.contains("### Hardware Progress By Area"));
    assert!(stdout.contains("| trackpad | 0 | 7 | 0.00% |"));
    assert!(stdout.contains("### Hardware Progress By Side"));
    assert!(stdout.contains("| right | 0 | 5 | 0.00% |"));
    assert!(stdout.contains("### Hardware Remaining"));
    assert!(stdout.contains("| vial_thumb_layer_taps | vial | both | requires_hardware |"));
}

#[test]
fn hardware_validation_markdown_report_lists_required_evidence() {
    let output = run_hardware_validation(&["--markdown"]);

    assert!(
        output.status.success(),
        "hardware validation markdown report failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest = hardware_validation_manifest_toml();
    let expected_total = manifest["checks"].as_array().unwrap().len();
    assert!(stdout.contains("## Real-Hardware Validation"));
    assert!(stdout.contains(&format!(
        "Hardware validation: 0/{expected_total} = 0.00% validated"
    )));
    assert!(stdout.contains("### Progress By Area"));
    assert!(stdout.contains("| Area | Validated | Total | Rate | Status counts |"));
    assert!(stdout.contains("| trackpad | 0 | 7 | 0.00% | `requires_hardware`=7 |"));
    assert!(stdout.contains("### Progress By Side"));
    assert!(stdout.contains("| Side | Validated | Total | Rate | Status counts |"));
    assert!(stdout.contains("| right | 0 | 5 | 0.00% | `requires_hardware`=5 |"));
    assert!(stdout.contains("### Checks"));
    assert!(stdout.contains(
        "| ID | Area | Side | Status | Requirement | Required evidence | Validated at | Tester | Firmware ref | Artifact/notes |"
    ));
    for required in [
        "iqs9151_right_i2c_identity",
        "left_trackpad_split_cursor_tap_scroll",
        "vial_thumb_layer_taps",
        "storage_reset_and_reflash",
    ] {
        assert!(
            stdout.contains(required),
            "hardware validation markdown report is missing {required}"
        );
    }
}

#[test]
fn hardware_validation_markdown_escapes_table_cells_and_shows_validation_evidence() {
    let manifest = r#"
[[checks]]
id = "pipe_check"
area = "trackpad"
side = "right"
requirement = """A | B
C"""
evidence = "Scope | log"
source = "docs/TRACKPAD_HARDWARE_CHECK.md"
status = "validated"
validated_at = "2026-05-29"
tester = "bench | tester"
firmware_ref = "v0.2.65 | 35b3f1f"
artifact_or_notes = "photo | serial log"
"#;
    let path = write_temp_file("hardware-validation-pipes", manifest);
    let output = run_hardware_validation(&["--manifest", path.to_str().unwrap(), "--markdown"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "hardware validation markdown escaped report failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hardware validation: 1/1 = 100.00% validated"));
    assert!(stdout.contains("A \\| B C"));
    assert!(stdout.contains("Scope \\| log"));
    assert!(stdout.contains("bench \\| tester"));
    assert!(stdout.contains("v0.2.65 \\| 35b3f1f"));
    assert!(stdout.contains("photo \\| serial log"));
}

#[test]
fn hardware_validation_requires_valid_source_anchors() {
    let manifest = r#"
[[checks]]
id = "bad_source"
area = "trackpad"
side = "right"
requirement = "A hardware-only behavior is documented."
evidence = "Run the documented hardware check."
source = "docs/TRACKPAD_HARDWARE_CHECK.md#missing-heading"
status = "requires_hardware"
"#;
    let path = write_temp_file("hardware-validation-bad-source", manifest);
    let output =
        run_hardware_validation(&["--manifest", path.to_str().unwrap(), "--require-classified"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        !output.status.success(),
        "bad hardware source link unexpectedly passed --require-classified\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("bad_source: source anchor #missing-heading was not found"),
        "bad hardware source link should explain the missing anchor"
    );
}

#[test]
fn hardware_validation_source_must_be_markdown() {
    let manifest = r#"
[[checks]]
id = "toml_source"
area = "trackpad"
side = "right"
requirement = "A hardware-only behavior is documented."
evidence = "Run the documented hardware check."
source = "Cargo.toml"
status = "requires_hardware"
"#;
    let path = write_temp_file("hardware-validation-toml-source", manifest);
    let output =
        run_hardware_validation(&["--manifest", path.to_str().unwrap(), "--require-classified"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        !output.status.success(),
        "non-Markdown hardware source link unexpectedly passed --require-classified"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("toml_source: source file 'Cargo.toml' must be Markdown"),
        "non-Markdown source should explain the file type error"
    );
}

#[test]
fn hardware_validation_markdown_anchor_generation_handles_common_headings() {
    let script = r###"
import importlib.util
from pathlib import Path
import sys
import tempfile

spec = importlib.util.spec_from_file_location("hardware_validation", "tools/hardware_validation.py")
hv = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = hv
spec.loader.exec_module(hv)

path = Path(tempfile.gettempdir()) / "lalapad-hardware-validation-anchors.md"
path.write_text(
    "# Heading ###\n"
    "## `Backtick Code`\n"
    "## [Link Label](https://example.com)!\n"
    "## Duplicate\n"
    "## Duplicate\n",
    encoding="utf-8",
)
try:
    print("\n".join(sorted(hv.markdown_anchors(path))))
finally:
    path.unlink(missing_ok=True)
"###;
    let output = run_python(script);

    assert!(
        output.status.success(),
        "hardware validation anchor parser failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "backtick-code",
        "duplicate",
        "duplicate-1",
        "heading",
        "link-label",
    ] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "anchor parser output is missing {expected:?}: {stdout}"
        );
    }
}

#[test]
fn hardware_validation_require_validated_rejects_malformed_manifest() {
    let manifest = r#"
checks = "not an array"
"#;
    let path = write_temp_file("hardware-validation-malformed", manifest);
    let output =
        run_hardware_validation(&["--manifest", path.to_str().unwrap(), "--require-validated"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        !output.status.success(),
        "malformed hardware manifest unexpectedly passed --require-validated\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checks must be an array"),
        "malformed hardware manifest should explain the schema error"
    );
}

#[test]
fn hardware_validation_evidence_overlay_can_validate_individual_checks() {
    let evidence = r#"
[[evidence]]
id = "iqs9151_right_i2c_identity"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
firmware_ref = "35b3f1f"
artifact_or_notes = "I2C scan found 0x56 and product register 0x1000 read 0x09bc."
"#;
    let path = write_temp_file("hardware-validation-evidence", evidence);
    let output = run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "hardware validation evidence overlay failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["total"].as_i64(), Some(12));
    assert_eq!(parsed["validated"].as_i64(), Some(1));
    assert_eq!(parsed["by_status"]["validated"].as_i64(), Some(1));
    assert_eq!(parsed["by_status"]["requires_hardware"].as_i64(), Some(11));
    assert_eq!(parsed["by_area"]["trackpad"]["total"].as_i64(), Some(7));
    assert_eq!(parsed["by_area"]["trackpad"]["validated"].as_i64(), Some(1));
    assert!(
        (parsed["by_area"]["trackpad"]["rate"].as_f64().unwrap() - (100.0 / 7.0)).abs() < 1e-12
    );
    assert_eq!(parsed["by_side"]["right"]["total"].as_i64(), Some(5));
    assert_eq!(parsed["by_side"]["right"]["validated"].as_i64(), Some(1));
    assert_eq!(parsed["by_side"]["right"]["rate"].as_f64(), Some(20.0));
    assert_eq!(parsed["errors"].as_array().unwrap().len(), 0);
}

#[test]
fn hardware_validation_can_require_matching_firmware_ref() {
    let evidence = r#"
[[evidence]]
id = "iqs9151_right_i2c_identity"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
firmware_ref = "35b3f1f"
artifact_or_notes = "I2C scan found 0x56 and product register 0x1000 read 0x09bc."
"#;
    let path = write_temp_file("hardware-validation-firmware-ref", evidence);
    let ok = run_hardware_validation(&[
        "--evidence",
        path.to_str().unwrap(),
        "--require-firmware-ref",
        "35b3f1f",
    ]);
    let bad = run_hardware_validation(&[
        "--evidence",
        path.to_str().unwrap(),
        "--require-firmware-ref",
        "v0.2.64",
    ]);
    let _ = std::fs::remove_file(&path);

    assert!(
        ok.status.success(),
        "matching firmware_ref should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ok.stdout),
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(
        !bad.status.success(),
        "mismatched firmware_ref unexpectedly passed"
    );
    assert!(
        String::from_utf8_lossy(&bad.stderr)
            .contains("validated firmware_ref '35b3f1f' does not match required 'v0.2.64'"),
        "mismatched firmware_ref should explain the stale evidence"
    );
}

#[test]
fn hardware_validation_requires_firmware_ref_for_validated_evidence() {
    let evidence = r#"
[[evidence]]
id = "iqs9151_right_i2c_identity"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
artifact_or_notes = "missing firmware_ref should not count"
"#;
    let path = write_temp_file("hardware-validation-missing-firmware-ref", evidence);
    let output = run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--json"]);
    let require_output =
        run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--require-classified"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "missing firmware_ref json report failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["validated"].as_i64(), Some(0));
    assert_eq!(parsed["classified"].as_bool(), Some(false));
    assert_eq!(parsed["by_area"]["trackpad"]["validated"].as_i64(), Some(0));
    assert_eq!(
        parsed["by_area"]["trackpad"]["by_status"]["validated"].as_i64(),
        Some(1)
    );
    assert_eq!(parsed["by_side"]["right"]["validated"].as_i64(), Some(0));
    assert_eq!(
        parsed["by_side"]["right"]["by_status"]["validated"].as_i64(),
        Some(1)
    );
    assert!(
        parsed["errors"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .contains("validated checks require evidence field(s): firmware_ref")
    );
    assert!(
        !require_output.status.success(),
        "missing firmware_ref unexpectedly passed --require-classified"
    );
}

#[test]
fn hardware_validation_firmware_ref_gate_allows_zero_validated_checks() {
    let output = run_hardware_validation(&["--require-firmware-ref", "35b3f1f"]);

    assert!(
        output.status.success(),
        "--require-firmware-ref should only constrain validated evidence; use --require-validated for all-check enforcement"
    );
}

#[test]
fn hardware_validation_can_generate_complete_evidence_template() {
    let output = run_hardware_validation(&["--evidence-template"]);

    assert!(
        output.status.success(),
        "hardware validation evidence template generation failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let generated: toml::Value = toml::from_str(&stdout).unwrap();
    let template_entries = generated["evidence"].as_array().unwrap();
    let manifest = hardware_validation_manifest_toml();
    let checks = manifest["checks"].as_array().unwrap();
    assert_eq!(template_entries.len(), checks.len());
    for check in checks {
        let check_id = check["id"].as_str().unwrap();
        let entry = template_entries
            .iter()
            .find(|entry| entry["id"].as_str() == Some(check_id))
            .unwrap_or_else(|| panic!("evidence template missing {check_id}"));
        assert_eq!(entry["status"].as_str(), Some("requires_hardware"));
        assert_eq!(entry["validated_at"].as_str(), Some(""));
        assert_eq!(entry["tester"].as_str(), Some(""));
        assert_eq!(entry["firmware_ref"].as_str(), Some(""));
        assert_eq!(entry["artifact_or_notes"].as_str(), Some(""));
        assert!(
            stdout.contains(check["requirement"].as_str().unwrap()),
            "evidence template should include requirement comments for {check_id}"
        );
    }

    let path = write_temp_file("hardware-validation-template", &stdout);
    let overlay_output = run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(&path);
    assert!(
        overlay_output.status.success(),
        "generated evidence template should be a valid non-progress overlay\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&overlay_output.stdout),
        String::from_utf8_lossy(&overlay_output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&overlay_output.stdout).unwrap();
    assert_eq!(parsed["validated"].as_i64(), Some(0));
    assert_eq!(parsed["classified"].as_bool(), Some(true));
}

#[test]
fn hardware_validation_evidence_template_can_prefill_firmware_ref() {
    let output =
        run_hardware_validation(&["--evidence-template", "--firmware-ref-template", "v0.2.66"]);

    assert!(
        output.status.success(),
        "hardware validation evidence template with firmware ref failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let generated: toml::Value = toml::from_str(&stdout).unwrap();
    let template_entries = generated["evidence"].as_array().unwrap();
    let manifest = hardware_validation_manifest_toml();
    assert_eq!(
        template_entries.len(),
        manifest["checks"].as_array().unwrap().len()
    );
    assert!(
        template_entries
            .iter()
            .all(|entry| entry["firmware_ref"].as_str() == Some("v0.2.66")),
        "all template entries should prefill the requested firmware_ref"
    );
}

#[test]
fn hardware_validation_rejects_template_firmware_ref_without_template_output() {
    let output = run_hardware_validation(&[
        "--manifest",
        "/tmp/lalapad-missing-hardware-validation.toml",
        "--firmware-ref-template",
        "v0.2.66",
    ]);

    assert!(
        !output.status.success(),
        "--firmware-ref-template without --evidence-template unexpectedly passed"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--firmware-ref-template can only be used with --evidence-template"),
        "invalid firmware_ref template usage should explain the required output mode"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("No such file"),
        "invalid firmware_ref template usage should fail before reading manifest files"
    );
}

#[test]
fn hardware_validation_does_not_count_incomplete_validated_evidence() {
    let evidence = r#"
[[evidence]]
id = "iqs9151_right_i2c_identity"
status = "validated"
validated_at = "2026-05-29"
artifact_or_notes = "missing tester should not count"
"#;
    let path = write_temp_file("hardware-validation-incomplete-evidence", evidence);
    let output = run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--json"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        output.status.success(),
        "incomplete hardware evidence json report failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["validated"].as_i64(), Some(0));
    assert_eq!(parsed["classified"].as_bool(), Some(false));
    assert!(
        parsed["errors"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .contains("validated checks require evidence field(s): tester")
    );

    let require_output =
        run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--require-classified"]);
    assert!(
        !require_output.status.success(),
        "incomplete hardware evidence unexpectedly passed --require-classified"
    );
}

#[test]
fn hardware_validation_evidence_overlay_rejects_unknown_and_duplicate_ids() {
    let evidence = r#"
[[evidence]]
id = "unknown_check"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
firmware_ref = "35b3f1f"
artifact_or_notes = "unknown"

[[evidence]]
id = "vial_thumb_layer_taps"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
firmware_ref = "35b3f1f"
artifact_or_notes = "first"

[[evidence]]
id = "vial_thumb_layer_taps"
status = "validated"
validated_at = "2026-05-29"
tester = "hardware bench"
firmware_ref = "35b3f1f"
artifact_or_notes = "duplicate"
"#;
    let path = write_temp_file("hardware-validation-bad-evidence", evidence);
    let output =
        run_hardware_validation(&["--evidence", path.to_str().unwrap(), "--require-classified"]);
    let _ = std::fs::remove_file(&path);

    assert!(
        !output.status.success(),
        "bad hardware evidence unexpectedly passed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown_check: evidence references unknown hardware check"));
    assert!(stderr.contains("vial_thumb_layer_taps: duplicate evidence entry"));
}

#[test]
fn local_validation_entrypoints_match_ci_gates() {
    assert!(
        MAKEFILE_TOML.contains("--require-porting-complete"),
        "cargo make porting-coverage should require complete implementation status"
    );
    assert!(
        MAKEFILE_TOML.contains("[tasks.migration-status]")
            && MAKEFILE_TOML.contains("tools/migration_status.py")
            && MAKEFILE_TOML.contains("--require-software-complete")
            && MAKEFILE_TOML.contains("--require-hardware-classified"),
        "cargo make migration-status should expose the combined release dashboard gate"
    );
    assert!(
        MAKEFILE_TOML.contains("[tasks.hardware-validation]")
            && MAKEFILE_TOML.contains("tools/hardware_validation.py")
            && MAKEFILE_TOML.contains("--require-classified"),
        "Makefile.toml should expose the hardware validation classification gate"
    );
    assert!(
        MAKEFILE_TOML.contains("[tasks.hardware-validation-report]")
            && MAKEFILE_TOML.contains("--markdown"),
        "Makefile.toml should expose a hardware validation markdown report"
    );
    assert!(
        MAKEFILE_TOML.contains("[tasks.hardware-validation-evidence-template]")
            && MAKEFILE_TOML.contains("--evidence-template"),
        "Makefile.toml should expose a full hardware evidence template generator"
    );
    assert!(
        include_str!("../.gitignore").contains("hardware-validation-evidence*.toml"),
        "local generated hardware evidence overlays should be ignored by default"
    );
    for required in [
        "--require-porting-complete",
        "tools/migration_status.py --require-zmk-source --require-software-complete --require-hardware-classified",
        "tools/hardware_validation.py --require-classified",
        "tools/hardware_validation.py --markdown",
        "tools/hardware_validation.py --evidence-template",
        "tools/hardware_validation.py --evidence-template --firmware-ref-template <tag-or-commit>",
        "tools/hardware_validation.py --evidence path/to/evidence.toml --markdown",
        "tools/hardware_validation.py --evidence path/to/evidence.toml --require-validated --require-firmware-ref <tag-or-commit>",
        "tools/hardware_validation_manifest.toml",
        "tools/hardware_validation_evidence.example.toml",
    ] {
        assert!(
            PULL_REQUEST_TEMPLATE_MD.contains(required),
            "PR template is missing validation item {required:?}"
        );
    }
    assert!(
        FIRMWARE_WORKFLOW_YAML.contains("tools/hardware_validation_evidence.example.toml"),
        "firmware CI path filters should include the hardware evidence template"
    );
    assert!(
        FIRMWARE_WORKFLOW_YAML.contains("tools/migration_status.py")
            && FIRMWARE_WORKFLOW_YAML.contains("--require-software-complete")
            && FIRMWARE_WORKFLOW_YAML.contains("--require-hardware-classified")
            && FIRMWARE_WORKFLOW_YAML.contains("--markdown >> \"$GITHUB_STEP_SUMMARY\""),
        "firmware CI should publish the combined migration status dashboard"
    );
    assert!(
        FIRMWARE_WORKFLOW_YAML.contains(".github/workflows/auto-tag.yml"),
        "firmware CI path filters should include auto-tag workflow changes covered by host parity tests"
    );
    assert!(
        FIRMWARE_WORKFLOW_YAML.contains("vendor/rmk-0.8.2/**"),
        "firmware CI path filters should include the local RMK patch that affects HID behavior"
    );
    assert!(
        AUTO_TAG_WORKFLOW_YAML.contains("vendor/rmk-0.8.2/**"),
        "auto-tag path filters should release firmware changes caused by the local RMK patch"
    );
    assert!(
        HARDWARE_VALIDATION_EVIDENCE_EXAMPLE_TOML
            .contains("tools/hardware_validation.py --evidence path/to/evidence.toml --require-firmware-ref <tag-or-commit>"),
        "hardware evidence example should document overlay report usage"
    );
    assert!(
        HARDWARE_VALIDATION_EVIDENCE_EXAMPLE_TOML
            .contains("tools/hardware_validation.py --evidence-template --firmware-ref-template <tag-or-commit>"),
        "hardware evidence example should document firmware_ref-prefilled template generation"
    );

    let manifest = hardware_validation_manifest_toml();
    for check in manifest["checks"].as_array().unwrap() {
        let source = check["source"].as_str().unwrap();
        let source_file = source.split('#').next().unwrap();
        assert!(
            FIRMWARE_WORKFLOW_YAML.contains(source_file),
            "firmware CI path filters should include hardware validation source file {source_file}"
        );
    }
}

#[test]
fn porting_coverage_includes_exact_rmk_inventory_gates() {
    let output = run_porting_coverage(&["--json"]);

    assert!(
        output.status.success(),
        "porting coverage failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = parsed["results"].as_array().unwrap();
    let by_kind = parsed["by_kind"].as_object().unwrap();
    let mut expected_by_kind: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for result in results {
        let kind = result["kind"].as_str().unwrap().to_string();
        let entry = expected_by_kind.entry(kind).or_insert((0, 0));
        entry.0 += result["passed"].as_i64().unwrap();
        entry.1 += result["total"].as_i64().unwrap();
    }
    assert_eq!(
        by_kind.len(),
        expected_by_kind.len(),
        "JSON coverage summary should include every result kind"
    );
    for (kind, (passed, total)) in expected_by_kind {
        let summary = by_kind
            .get(&kind)
            .unwrap_or_else(|| panic!("missing by_kind summary for {kind}"));
        assert_eq!(
            summary["passed"].as_i64(),
            Some(passed),
            "by_kind passed total drifted for {kind}"
        );
        assert_eq!(
            summary["total"].as_i64(),
            Some(total),
            "by_kind total drifted for {kind}"
        );
        if total == 0 {
            assert!(summary["rate"].is_null());
        } else {
            let expected_rate = passed as f64 / total as f64;
            assert!(
                (summary["rate"].as_f64().unwrap() - expected_rate).abs() < 1e-12,
                "by_kind rate drifted for {kind}"
            );
        }
    }
    let manifest = porting_coverage_manifest_toml();
    let status_counts = manifest_status_counts(&manifest);
    let expected_status_total: i64 = status_counts.values().sum();
    let expected_implemented_status_total: i64 =
        ["ported", "ported_by_behavior", "ported_by_config_image"]
            .iter()
            .map(|status| status_counts.get(*status).copied().unwrap_or_default())
            .sum();
    let porting_status = &parsed["porting_status"];
    assert_eq!(
        porting_status["total"].as_i64(),
        Some(expected_status_total),
        "porting status summary total should track explicit manifest statuses"
    );
    assert_eq!(
        porting_status["implemented"].as_i64(),
        Some(expected_implemented_status_total),
        "porting status summary should distinguish implemented statuses from remaining gaps"
    );
    assert_eq!(
        porting_status["remaining"].as_array().unwrap().len() as i64,
        expected_status_total - expected_implemented_status_total
    );
    for (status, count) in status_counts {
        assert_eq!(
            porting_status["by_status"][status.as_str()].as_i64(),
            Some(count),
            "porting status summary is missing manifest status {status}"
        );
    }
    assert!(
        manifest.get("source_regex_values").is_none(),
        "ZMK source coverage should use structured inventory gates instead of regex-only checks"
    );
    assert!(
        results
            .iter()
            .all(|result| result["kind"] != "zmk_source_regex"),
        "porting coverage emitted regex-only ZMK source checks"
    );
    let keyboard = keyboard_toml();
    let layers = keyboard["layout"]["layers"].as_integer().unwrap();
    let rows = keyboard["layout"]["rows"].as_integer().unwrap();
    let expected_keymap_shape_total = 1 + layers + layers * rows;
    let expected_combo_total = keyboard["behavior"]["combo"]["combos"]
        .as_array()
        .unwrap()
        .len() as i64;
    let expected_scenario_count = manifest["scenarios"].as_array().unwrap().len();

    let keymap_shape = results
        .iter()
        .find(|result| result["id"] == "keymap_shape_matches_layout")
        .expect("keymap shape coverage result is missing");
    assert_eq!(keymap_shape["kind"], "keymap_shape");
    assert_eq!(
        keymap_shape["passed"].as_i64(),
        Some(expected_keymap_shape_total)
    );
    assert_eq!(
        keymap_shape["total"].as_i64(),
        Some(expected_keymap_shape_total)
    );
    assert_eq!(keymap_shape["ok"], true);

    let combo_inventory = results
        .iter()
        .find(|result| result["id"] == "rmk_combo_set_matches_manifest")
        .expect("RMK combo inventory coverage result is missing");
    assert_eq!(combo_inventory["kind"], "combo_inventory");
    assert_eq!(
        combo_inventory["passed"].as_i64(),
        Some(expected_combo_total)
    );
    assert_eq!(
        combo_inventory["total"].as_i64(),
        Some(expected_combo_total)
    );
    assert_eq!(combo_inventory["ok"], true);

    for scenario in manifest["scenarios"].as_array().unwrap() {
        let scenario_id = scenario["id"].as_str().unwrap();
        let scenario_result = results
            .iter()
            .find(|result| result["id"] == scenario_id)
            .unwrap_or_else(|| panic!("RMK semantic scenario result is missing for {scenario_id}"));
        assert_eq!(scenario_result["kind"], "scenario");
        assert_eq!(scenario_result["ok"], true);
    }

    if default_zmk_config_dir().is_some() {
        let source_scenarios: Vec<_> = results
            .iter()
            .filter(|result| result["kind"] == "zmk_source_scenario")
            .collect();
        assert_eq!(source_scenarios.len(), expected_scenario_count);
        for scenario in manifest["scenarios"].as_array().unwrap() {
            let scenario_id = scenario["id"].as_str().unwrap();
            let source_scenario = results
                .iter()
                .find(|result| result["id"] == format!("zmk_source.scenario.{scenario_id}"))
                .unwrap_or_else(|| {
                    panic!("ZMK source semantic scenario result is missing for {scenario_id}")
                });
            assert_eq!(source_scenario["kind"], "zmk_source_scenario");
            assert_eq!(source_scenario["ok"], true);
        }

        let expected_layer_resolution_total =
            rows * keyboard["layout"]["cols"].as_integer().unwrap();
        for resolution_id in [
            "zmk_source.layer1_resolution",
            "zmk_source.layer2_resolution",
            "zmk_source.tri_layer_resolution",
        ] {
            let layer_resolution = results
                .iter()
                .find(|result| result["id"] == resolution_id)
                .unwrap_or_else(|| {
                    panic!("ZMK source layer resolution result is missing for {resolution_id}")
                });
            assert_eq!(layer_resolution["kind"], "zmk_source_layer_resolution");
            assert_eq!(
                layer_resolution["passed"].as_i64(),
                Some(expected_layer_resolution_total)
            );
            assert_eq!(
                layer_resolution["total"].as_i64(),
                Some(expected_layer_resolution_total)
            );
            assert_eq!(layer_resolution["ok"], true);
        }

        let layer_inventory = results
            .iter()
            .find(|result| result["id"] == "zmk_source.keymap_layer_inventory")
            .expect("ZMK keymap layer inventory coverage result is missing");
        assert_eq!(layer_inventory["kind"], "zmk_source_inventory");
        assert_eq!(layer_inventory["passed"].as_i64(), Some(layers));
        assert_eq!(layer_inventory["total"].as_i64(), Some(layers));
        assert_eq!(layer_inventory["ok"], true);

        let behavior_inventory = results
            .iter()
            .find(|result| result["id"] == "zmk_source.behavior_inventory")
            .expect("ZMK behavior inventory coverage result is missing");
        let expected_behavior_nodes =
            porting_coverage_manifest_toml()["source_inventory"]["behavior_nodes"]
                .as_array()
                .unwrap()
                .len() as i64;
        assert_eq!(behavior_inventory["kind"], "zmk_source_inventory");
        assert_eq!(
            behavior_inventory["passed"].as_i64(),
            Some(expected_behavior_nodes)
        );
        assert_eq!(
            behavior_inventory["total"].as_i64(),
            Some(expected_behavior_nodes)
        );
        assert_eq!(behavior_inventory["ok"], true);

        for behavior_property in
            porting_coverage_manifest_toml()["source_inventory"]["behavior_properties"]
                .as_array()
                .unwrap()
        {
            let source_block = behavior_property["source_block"].as_str().unwrap();
            let expected_properties =
                behavior_property["expected"].as_array().unwrap().len() as i64;
            let property_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.behavior_properties.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!("ZMK behavior property inventory coverage result is missing for {source_block}")
                });
            assert_eq!(property_inventory["kind"], "zmk_inventory");
            assert_eq!(
                property_inventory["passed"].as_i64(),
                Some(expected_properties)
            );
            assert_eq!(
                property_inventory["total"].as_i64(),
                Some(expected_properties)
            );
            assert_eq!(property_inventory["ok"], true);
        }

        for combo_property in
            porting_coverage_manifest_toml()["source_inventory"]["combo_properties"]
                .as_array()
                .unwrap()
        {
            let source_block = combo_property["source_block"].as_str().unwrap();
            let expected_properties = combo_property["expected"].as_array().unwrap().len() as i64;
            let property_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.combo_properties.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK combo property inventory coverage result is missing for {source_block}"
                    )
                });
            assert_eq!(property_inventory["kind"], "zmk_inventory");
            assert_eq!(
                property_inventory["passed"].as_i64(),
                Some(expected_properties)
            );
            assert_eq!(
                property_inventory["total"].as_i64(),
                Some(expected_properties)
            );
            assert_eq!(property_inventory["ok"], true);
        }

        for mirror in porting_coverage_manifest_toml()["zmk_behavior_mirrors"]
            .as_array()
            .unwrap()
        {
            let mirror_id = mirror["id"].as_str().unwrap();
            let mirror_result = results
                .iter()
                .find(|result| result["id"] == mirror_id)
                .unwrap_or_else(|| {
                    panic!("ZMK behavior mirror coverage result is missing for {mirror_id}")
                });
            assert_eq!(mirror_result["kind"], "zmk_behavior_mirror");
            assert_eq!(mirror_result["passed"].as_i64(), Some(2));
            assert_eq!(mirror_result["total"].as_i64(), Some(2));
            assert_eq!(mirror_result["ok"], true);
        }

        let file_inventory = results
            .iter()
            .find(|result| result["id"] == "zmk_source.file_inventory")
            .expect("ZMK source file inventory coverage result is missing");
        let expected_source_files =
            porting_coverage_manifest_toml()["source_inventory"]["source_files"]
                .as_array()
                .unwrap()
                .len() as i64;
        assert_eq!(file_inventory["kind"], "zmk_inventory");
        assert_eq!(
            file_inventory["passed"].as_i64(),
            Some(expected_source_files)
        );
        assert_eq!(
            file_inventory["total"].as_i64(),
            Some(expected_source_files)
        );
        assert_eq!(file_inventory["ok"], true);

        let repo_file_inventory = results
            .iter()
            .find(|result| result["id"] == "zmk_source.repo_file_inventory")
            .expect("ZMK repo file inventory coverage result is missing");
        let expected_repo_files = porting_coverage_manifest_toml()["source_inventory"]["repo_files"]
            .as_array()
            .unwrap()
            .len() as i64;
        assert_eq!(repo_file_inventory["kind"], "zmk_inventory");
        assert_eq!(
            repo_file_inventory["passed"].as_i64(),
            Some(expected_repo_files)
        );
        assert_eq!(
            repo_file_inventory["total"].as_i64(),
            Some(expected_repo_files)
        );
        assert_eq!(repo_file_inventory["ok"], true);

        for include_file in porting_coverage_manifest_toml()["source_inventory"]["include_files"]
            .as_array()
            .unwrap()
        {
            let source_file = include_file["source_file"].as_str().unwrap();
            let expected_includes = include_file["expected"].as_array().unwrap().len() as i64;
            let include_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.include_inventory.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!("ZMK include inventory coverage result is missing for {source_file}")
                });
            assert_eq!(include_inventory["kind"], "zmk_inventory");
            assert_eq!(
                include_inventory["passed"].as_i64(),
                Some(expected_includes)
            );
            assert_eq!(include_inventory["total"].as_i64(), Some(expected_includes));
            assert_eq!(include_inventory["ok"], true);
        }

        for kconfig_file in porting_coverage_manifest_toml()["source_inventory"]["kconfig_entries"]
            .as_array()
            .unwrap()
        {
            let source_file = kconfig_file["source_file"].as_str().unwrap();
            let expected_entries = kconfig_file["expected"].as_array().unwrap().len() as i64;
            let kconfig_inventory = results
                .iter()
                .find(|result| result["id"] == format!("zmk_source.kconfig_entries.{source_file}"))
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK Kconfig entry inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(kconfig_inventory["kind"], "zmk_inventory");
            assert_eq!(kconfig_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(kconfig_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(kconfig_inventory["ok"], true);
        }

        for kconfig_file in porting_coverage_manifest_toml()["source_inventory"]["kconfig_lines"]
            .as_array()
            .unwrap()
        {
            let source_file = kconfig_file["source_file"].as_str().unwrap();
            let expected_lines = kconfig_file["expected"].as_array().unwrap().len() as i64;
            let kconfig_inventory = results
                .iter()
                .find(|result| result["id"] == format!("zmk_source.kconfig_lines.{source_file}"))
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK Kconfig line inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(kconfig_inventory["kind"], "zmk_inventory");
            assert_eq!(kconfig_inventory["passed"].as_i64(), Some(expected_lines));
            assert_eq!(kconfig_inventory["total"].as_i64(), Some(expected_lines));
            assert_eq!(kconfig_inventory["ok"], true);
        }

        for kconfig_file in
            porting_coverage_manifest_toml()["source_inventory"]["disabled_kconfig_lines"]
                .as_array()
                .unwrap()
        {
            let source_file = kconfig_file["source_file"].as_str().unwrap();
            let expected_lines = kconfig_file["expected"].as_array().unwrap().len() as i64;
            let kconfig_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.disabled_kconfig_lines.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK disabled Kconfig line inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(kconfig_inventory["kind"], "zmk_inventory");
            assert_eq!(kconfig_inventory["passed"].as_i64(), Some(expected_lines));
            assert_eq!(kconfig_inventory["total"].as_i64(), Some(expected_lines));
            assert_eq!(kconfig_inventory["ok"], true);
        }

        for define_file in porting_coverage_manifest_toml()["source_inventory"]["define_entries"]
            .as_array()
            .unwrap()
        {
            let source_file = define_file["source_file"].as_str().unwrap();
            let expected_entries = define_file["expected"].as_array().unwrap().len() as i64;
            let define_inventory = results
                .iter()
                .find(|result| result["id"] == format!("zmk_source.define_entries.{source_file}"))
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK define entry inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(define_inventory["kind"], "zmk_inventory");
            assert_eq!(define_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(define_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(define_inventory["ok"], true);
        }

        for alias_file in porting_coverage_manifest_toml()["source_inventory"]["dts_aliases"]
            .as_array()
            .unwrap()
        {
            let source_file = alias_file["source_file"].as_str().unwrap();
            let source_block = alias_file["source_block"].as_str().unwrap();
            let expected_entries = alias_file["expected"].as_array().unwrap().len() as i64;
            let alias_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.dts_aliases.{source_file}.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!("ZMK DTS alias inventory coverage result is missing for {source_file}")
                });
            assert_eq!(alias_inventory["kind"], "zmk_inventory");
            assert_eq!(alias_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(alias_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(alias_inventory["ok"], true);
        }

        for node_inventory in
            porting_coverage_manifest_toml()["source_inventory"]["dts_node_inventories"]
                .as_array()
                .unwrap()
        {
            let source_file = node_inventory["source_file"].as_str().unwrap();
            let source_block = node_inventory["source_block"].as_str().unwrap();
            let expected_entries = node_inventory["expected"].as_array().unwrap().len() as i64;
            let inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.dts_nodes.{source_file}.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK DTS node inventory coverage result is missing for {source_file} {source_block}"
                    )
                });
            assert_eq!(inventory["kind"], "zmk_inventory");
            assert_eq!(inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(inventory["ok"], true);
        }

        for layout_file in
            porting_coverage_manifest_toml()["source_inventory"]["physical_layout_attrs"]
                .as_array()
                .unwrap()
        {
            let source_file = layout_file["source_file"].as_str().unwrap();
            let expected_entries = layout_file["expected"].as_array().unwrap().len() as i64;
            let layout_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.physical_layout_attrs.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK physical layout attr inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(layout_inventory["kind"], "zmk_inventory");
            assert_eq!(layout_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(layout_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(layout_inventory["ok"], true);
        }

        for binding_file in
            porting_coverage_manifest_toml()["source_inventory"]["input_behavior_bindings"]
                .as_array()
                .unwrap()
        {
            let source_block = binding_file["source_block"].as_str().unwrap();
            let expected_entries = binding_file["expected"].as_array().unwrap().len() as i64;
            let binding_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.input_behavior_bindings.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK input behavior binding inventory coverage result is missing for {source_block}"
                    )
                });
            assert_eq!(binding_inventory["kind"], "zmk_inventory");
            assert_eq!(binding_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(binding_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(binding_inventory["ok"], true);
        }

        for listener in porting_coverage_manifest_toml()["source_inventory"]["input_listeners"]
            .as_array()
            .unwrap()
        {
            let source_file = listener["source_file"].as_str().unwrap();
            let source_block = listener["source_block"].as_str().unwrap();
            let expected_entries = listener["expected"].as_array().unwrap().len() as i64;
            let listener_inventory = results
                .iter()
                .find(|result| {
                    result["id"]
                        == format!("zmk_source.input_listeners.{source_file}.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK input listener inventory coverage result is missing for {source_file} {source_block}"
                    )
                });
            assert_eq!(listener_inventory["kind"], "zmk_inventory");
            assert_eq!(
                listener_inventory["passed"].as_i64(),
                Some(expected_entries)
            );
            assert_eq!(listener_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(listener_inventory["ok"], true);
        }

        for dts_property_block in
            porting_coverage_manifest_toml()["source_inventory"]["dts_properties"]
                .as_array()
                .unwrap()
        {
            let source_file = dts_property_block["source_file"].as_str().unwrap();
            let source_block = dts_property_block["source_block"].as_str().unwrap();
            let expected_entries = dts_property_block["expected"].as_array().unwrap().len() as i64;
            let property_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.dts_properties.{source_file}.{source_block}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK DTS property inventory coverage result is missing for {source_file} {source_block}"
                    )
                });
            assert_eq!(property_inventory["kind"], "zmk_inventory");
            assert_eq!(
                property_inventory["passed"].as_i64(),
                Some(expected_entries)
            );
            assert_eq!(property_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(property_inventory["ok"], true);
        }

        for gpio_property in porting_coverage_manifest_toml()["source_inventory"]["gpio_properties"]
            .as_array()
            .unwrap()
        {
            let source_file = gpio_property["source_file"].as_str().unwrap();
            let source_block = gpio_property["source_block"].as_str().unwrap();
            let source_property = gpio_property["source_property"].as_str().unwrap();
            let expected_entries = gpio_property["expected"].as_array().unwrap().len() as i64;
            let gpio_inventory = results
                .iter()
                .find(|result| {
                    result["id"]
                        == format!(
                            "zmk_source.gpio_properties.{source_file}.{source_block}.{source_property}"
                        )
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK GPIO property inventory coverage result is missing for {source_file} {source_block}.{source_property}"
                    )
                });
            assert_eq!(gpio_inventory["kind"], "zmk_inventory");
            assert_eq!(gpio_inventory["passed"].as_i64(), Some(expected_entries));
            assert_eq!(gpio_inventory["total"].as_i64(), Some(expected_entries));
            assert_eq!(gpio_inventory["ok"], true);
        }

        let west_inventory = results
            .iter()
            .find(|result| result["id"] == "zmk_source.west_manifest")
            .expect("ZMK west manifest inventory coverage result is missing");
        let expected_west_items =
            porting_coverage_manifest_toml()["source_inventory"]["west_manifest"]
                .as_array()
                .unwrap()
                .len() as i64;
        assert_eq!(west_inventory["kind"], "zmk_inventory");
        assert_eq!(west_inventory["passed"].as_i64(), Some(expected_west_items));
        assert_eq!(west_inventory["total"].as_i64(), Some(expected_west_items));
        assert_eq!(west_inventory["ok"], true);

        for build_file in porting_coverage_manifest_toml()["source_inventory"]["build_files"]
            .as_array()
            .unwrap()
        {
            let source_file = build_file["source_file"].as_str().unwrap();
            let expected_items = build_file["expected"].as_array().unwrap().len() as i64;
            let build_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.build_file_inventory.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!("ZMK build file inventory coverage result is missing for {source_file}")
                });
            assert_eq!(build_inventory["kind"], "zmk_inventory");
            assert_eq!(build_inventory["passed"].as_i64(), Some(expected_items));
            assert_eq!(build_inventory["total"].as_i64(), Some(expected_items));
            assert_eq!(build_inventory["ok"], true);
        }

        for workflow_file in porting_coverage_manifest_toml()["source_inventory"]["workflow_files"]
            .as_array()
            .unwrap()
        {
            let source_file = workflow_file["source_file"].as_str().unwrap();
            let expected_items = workflow_file["expected"].as_array().unwrap().len() as i64;
            let workflow_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.workflow_file_inventory.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK workflow file inventory coverage result is missing for {source_file}"
                    )
                });
            assert_eq!(workflow_inventory["kind"], "zmk_inventory");
            assert_eq!(workflow_inventory["passed"].as_i64(), Some(expected_items));
            assert_eq!(workflow_inventory["total"].as_i64(), Some(expected_items));
            assert_eq!(workflow_inventory["ok"], true);
        }

        for json_file in porting_coverage_manifest_toml()["source_inventory"]["json_files"]
            .as_array()
            .unwrap()
        {
            let source_file = json_file["source_file"].as_str().unwrap();
            let expected_items = json_file["expected"].as_array().unwrap().len() as i64;
            let json_inventory = results
                .iter()
                .find(|result| {
                    result["id"] == format!("zmk_source.json_file_inventory.{source_file}")
                })
                .unwrap_or_else(|| {
                    panic!("ZMK JSON file inventory coverage result is missing for {source_file}")
                });
            assert_eq!(json_inventory["kind"], "zmk_inventory");
            assert_eq!(json_inventory["passed"].as_i64(), Some(expected_items));
            assert_eq!(json_inventory["total"].as_i64(), Some(expected_items));
            assert_eq!(json_inventory["ok"], true);
        }

        for json_layout in
            porting_coverage_manifest_toml()["source_inventory"]["json_layout_entries"]
                .as_array()
                .unwrap()
        {
            let source_file = json_layout["source_file"].as_str().unwrap();
            let layout_name = json_layout["layout_name"].as_str().unwrap();
            let expected_items = json_layout["expected"].as_array().unwrap().len() as i64;
            let layout_inventory = results
                .iter()
                .find(|result| {
                    result["id"]
                        == format!("zmk_source.json_layout_entries.{source_file}.{layout_name}")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ZMK JSON layout entry coverage result is missing for {source_file}:{layout_name}"
                    )
                });
            assert_eq!(layout_inventory["kind"], "zmk_inventory");
            assert_eq!(layout_inventory["passed"].as_i64(), Some(expected_items));
            assert_eq!(layout_inventory["total"].as_i64(), Some(expected_items));
            assert_eq!(layout_inventory["ok"], true);
        }
    }

    for byte_array in porting_coverage_manifest_toml()["rust_byte_arrays"]
        .as_array()
        .unwrap()
    {
        let array_id = byte_array["id"].as_str().unwrap();
        let inventory = results
            .iter()
            .find(|result| result["id"] == array_id)
            .unwrap_or_else(|| panic!("Rust byte array coverage result is missing for {array_id}"));
        assert_eq!(inventory["kind"], "rust_byte_array");
        assert_eq!(inventory["passed"].as_i64(), Some(2));
        assert_eq!(inventory["total"].as_i64(), Some(2));
        assert_eq!(inventory["ok"], true);
    }

    for const_inventory in porting_coverage_manifest_toml()["rust_const_inventories"]
        .as_array()
        .unwrap()
    {
        let inventory_id = const_inventory["id"].as_str().unwrap();
        let expected_len = const_inventory["expected"].as_array().unwrap().len() as i64;
        let inventory = results
            .iter()
            .find(|result| result["id"] == inventory_id)
            .unwrap_or_else(|| {
                panic!("Rust const inventory coverage result is missing for {inventory_id}")
            });
        assert_eq!(inventory["kind"], "rust_const_inventory");
        assert_eq!(inventory["passed"].as_i64(), Some(expected_len));
        assert_eq!(inventory["total"].as_i64(), Some(expected_len));
        assert_eq!(inventory["ok"], true);
    }

    let register_porting_inventory = results
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_register_address_classification")
        .expect("IQS9151 upstream register classification coverage result is missing");
    let expected_register_count =
        porting_coverage_manifest_toml()["source_inventory"]["iqs9151_register_addresses"]
            .as_array()
            .unwrap()
            .len() as i64;
    assert_eq!(
        register_porting_inventory["kind"],
        "iqs9151_register_porting"
    );
    assert_eq!(
        register_porting_inventory["passed"].as_i64(),
        Some(expected_register_count)
    );
    assert_eq!(
        register_porting_inventory["total"].as_i64(),
        Some(expected_register_count)
    );
    assert_eq!(register_porting_inventory["ok"], true);

    for register in porting_coverage_manifest_toml()["iqs9151_register_porting"]
        .as_array()
        .unwrap()
    {
        let source_const = register["source_const"].as_str().unwrap();
        let register_result = results
            .iter()
            .find(|result| result["id"] == format!("iqs9151_register_porting.{source_const}"))
            .unwrap_or_else(|| {
                panic!("IQS9151 register porting result is missing for {source_const}")
            });
        assert_eq!(register_result["kind"], "iqs9151_register_porting");
        assert_eq!(register_result["passed"].as_i64(), Some(2));
        assert_eq!(register_result["total"].as_i64(), Some(2));
        assert_eq!(register_result["ok"], true);
    }

    let bit_porting_inventory = results
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_bit_flag_classification")
        .expect("IQS9151 upstream bit flag classification coverage result is missing");
    let expected_bit_count =
        porting_coverage_manifest_toml()["source_inventory"]["iqs9151_bit_flags"]
            .as_array()
            .unwrap()
            .len() as i64;
    assert_eq!(bit_porting_inventory["kind"], "iqs9151_bit_porting");
    assert_eq!(
        bit_porting_inventory["passed"].as_i64(),
        Some(expected_bit_count)
    );
    assert_eq!(
        bit_porting_inventory["total"].as_i64(),
        Some(expected_bit_count)
    );
    assert_eq!(bit_porting_inventory["ok"], true);

    for bit in porting_coverage_manifest_toml()["iqs9151_bit_porting"]
        .as_array()
        .unwrap()
    {
        let source_const = bit["source_const"].as_str().unwrap();
        let bit_result = results
            .iter()
            .find(|result| result["id"] == format!("iqs9151_bit_porting.{source_const}"))
            .unwrap_or_else(|| {
                panic!("IQS9151 bit flag porting result is missing for {source_const}")
            });
        assert_eq!(bit_result["kind"], "iqs9151_bit_porting");
        assert_eq!(bit_result["passed"].as_i64(), Some(2));
        assert_eq!(bit_result["total"].as_i64(), Some(2));
        assert_eq!(bit_result["ok"], true);
    }
}

#[test]
fn porting_coverage_rejects_source_layer_resolution_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

source_layers = [
    [["A", "B"], ["C", "D"]],
    [["_", "Kp7"], ["_", "Kp0"]],
    [["Home", "_"], ["PageDown", "_"]],
    [["_", "User0"], ["_", "_"]],
]
ok_keyboard = {
    "layout": {
        "rows": 2,
        "cols": 2,
        "keymap": source_layers,
    },
}
changed_keyboard = {
    "layout": {
        "rows": 2,
        "cols": 2,
        "keymap": [
            [["A", "B"], ["C", "D"]],
            [["_", "Kp8"], ["_", "Kp0"]],
            [["Home", "_"], ["PageDown", "_"]],
            [["_", "User0"], ["_", "_"]],
        ],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

print(json.dumps({
    "ok": pack(pc.check_zmk_source_layer_resolution(ok_keyboard, source_layers)),
    "changed": pack(pc.check_zmk_source_layer_resolution(changed_keyboard, source_layers)),
}))
"#,
    );

    assert!(
        output.status.success(),
        "source layer resolution parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for result in parsed["ok"].as_array().unwrap() {
        assert_eq!(result["kind"], "zmk_source_layer_resolution");
        assert_eq!(result["passed"].as_i64(), Some(4));
        assert_eq!(result["total"].as_i64(), Some(4));
        assert_eq!(result["ok"], true);
    }

    let changed = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "zmk_source.layer1_resolution")
        .expect("layer 1 resolution result is missing");
    assert_eq!(changed["kind"], "zmk_source_layer_resolution");
    assert_eq!(changed["passed"].as_i64(), Some(3));
    assert_eq!(changed["total"].as_i64(), Some(4));
    assert_eq!(changed["ok"], false);
    assert!(
        changed["message"]
            .as_str()
            .unwrap()
            .contains("r0c1: source expected 'Kp7', RMK got 'Kp8'")
    );
}

#[test]
fn porting_coverage_rejects_source_scenario_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

config = {
    "behavior": {"tri_layer": {"lower": 1, "upper": 2, "adjust": 3}},
}
manifest = {
    "scenarios": [
        {
            "id": "space_hold_u",
            "hold": {"row": 0, "col": 0, "expected_action": "LT(1, Space, FAST_LAYER)", "activates_layer": 1},
            "tap": {"row": 0, "col": 1},
            "expected_output": "Kp7",
        },
        {
            "id": "tri_layer_fallback",
            "holds": [
                {"row": 0, "col": 0, "expected_action": "LT(1, Space, FAST_LAYER)", "activates_layer": 1},
                {"row": 0, "col": 2, "expected_action": "LT(2, Enter, FAST_LAYER)", "activates_layer": 2},
            ],
            "tap": {"row": 1, "col": 1},
            "expected_output": "Kp0",
        },
    ],
}
source_layers = [
    [["LT(1, Space, FAST_LAYER)", "U", "LT(2, Enter, FAST_LAYER)"], ["H", "J", "K"]],
    [["_", "Kp7", "_"], ["_", "Kp0", "_"]],
    [["_", "Home", "_"], ["_", "_", "_"]],
    [["_", "User0", "_"], ["_", "_", "_"]],
]
changed_layers = [[list(row) for row in layer] for layer in source_layers]
changed_layers[1][0][1] = "Kp8"

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

print(json.dumps({
    "ok": pack(pc.check_zmk_source_scenarios(manifest, config, source_layers)),
    "changed": pack(pc.check_zmk_source_scenarios(manifest, config, changed_layers)),
}))
"#,
    );

    assert!(
        output.status.success(),
        "source scenario parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["ok"][0]["kind"], "zmk_source_scenario");
    assert_eq!(parsed["ok"][0]["passed"].as_i64(), Some(2));
    assert_eq!(parsed["ok"][0]["total"].as_i64(), Some(2));
    assert_eq!(parsed["ok"][0]["ok"], true);
    assert_eq!(parsed["ok"][1]["passed"].as_i64(), Some(3));
    assert_eq!(parsed["ok"][1]["total"].as_i64(), Some(3));
    assert_eq!(parsed["ok"][1]["ok"], true);

    let changed = &parsed["changed"][0];
    assert_eq!(changed["kind"], "zmk_source_scenario");
    assert_eq!(changed["passed"].as_i64(), Some(1));
    assert_eq!(changed["total"].as_i64(), Some(2));
    assert_eq!(changed["ok"], false);
    assert!(
        changed["message"]
            .as_str()
            .unwrap()
            .contains("source output expected 'Kp7', got 'Kp8'")
    );
}

#[test]
fn porting_coverage_rejects_zmk_behavior_mirror_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

source = '''
&mt { quick-tap-ms = <200>; };
&lt { quick-tap-ms = <200>; };
/ {
    behaviors {
        mt2: mod_tap2 {
            flavor = "tap-preferred";
            tapping-term-ms = <200>;
            quick-tap-ms = <200>;
            require-prior-idle-ms = <125>;
        };
    };
};
'''
manifest = {
    "zmk_behavior_mirrors": [{
        "id": "prior_idle",
        "source_id": "zmk_mt2_require_prior_idle_ms",
        "source_expected": 125,
        "target_path": "behavior.morse.prior_idle_time",
        "transform": "ms_string",
    }],
}
ok_keyboard = {"behavior": {"morse": {"prior_idle_time": "125ms"}}}
changed_keyboard = {"behavior": {"morse": {"prior_idle_time": "200ms"}}}
changed_source = source.replace("require-prior-idle-ms = <125>;", "require-prior-idle-ms = <100>;")

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

print(json.dumps({
    "ok": pack(pc.check_zmk_behavior_mirrors(manifest, ok_keyboard, source)),
    "changed_target": pack(pc.check_zmk_behavior_mirrors(manifest, changed_keyboard, source)),
    "changed_source": pack(pc.check_zmk_behavior_mirrors(manifest, ok_keyboard, changed_source)),
}))
"#,
    );

    assert!(
        output.status.success(),
        "ZMK behavior mirror check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["ok"][0]["kind"], "zmk_behavior_mirror");
    assert_eq!(parsed["ok"][0]["passed"].as_i64(), Some(2));
    assert_eq!(parsed["ok"][0]["total"].as_i64(), Some(2));
    assert_eq!(parsed["ok"][0]["ok"], true);

    let changed_target = &parsed["changed_target"][0];
    assert_eq!(changed_target["passed"].as_i64(), Some(1));
    assert_eq!(changed_target["total"].as_i64(), Some(2));
    assert_eq!(changed_target["ok"], false);
    assert!(
        changed_target["message"]
            .as_str()
            .unwrap()
            .contains("target behavior.morse.prior_idle_time expected '125ms', got '200ms'")
    );

    let changed_source = &parsed["changed_source"][0];
    assert_eq!(changed_source["passed"].as_i64(), Some(0));
    assert_eq!(changed_source["total"].as_i64(), Some(2));
    assert_eq!(changed_source["ok"], false);
    assert!(
        changed_source["message"]
            .as_str()
            .unwrap()
            .contains("source zmk_mt2_require_prior_idle_ms expected 125, got 100")
    );
}

#[test]
fn porting_coverage_rejects_duplicate_rmk_combos() {
    let original =
        "  { actions = [\"D\", \"F\"], output = \"Language2\", layer = 0 },\n]\n\n[host]";
    let mutated = "  { actions = [\"D\", \"F\"], output = \"Language2\", layer = 0 },\n  { actions = [\"Q\", \"W\"], output = \"Escape\", layer = 0 },\n]\n\n[host]";
    let keyboard = KEYBOARD_TOML.replace(original, mutated);
    assert_ne!(keyboard, KEYBOARD_TOML);

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "lalapad-duplicate-combos-{}.toml",
        std::process::id()
    ));
    std::fs::write(&path, keyboard).unwrap();

    let output = Command::new("python3")
        .arg("tools/porting_coverage.py")
        .arg("--keyboard-toml")
        .arg(&path)
        .arg("--zmk-keymap")
        .arg("/nonexistent/lalapadgen2.keymap")
        .arg("--json")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();

    assert!(
        !output.status.success(),
        "duplicate combo coverage unexpectedly passed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = parsed["results"].as_array().unwrap();
    let combo_inventory = results
        .iter()
        .find(|result| result["id"] == "rmk_combo_set_matches_manifest")
        .expect("RMK combo inventory coverage result is missing");
    assert_eq!(combo_inventory["kind"], "combo_inventory");
    assert_eq!(combo_inventory["passed"].as_i64(), Some(4));
    assert_eq!(combo_inventory["total"].as_i64(), Some(5));
    assert_eq!(combo_inventory["ok"], false);
    assert!(
        combo_inventory["message"]
            .as_str()
            .unwrap()
            .contains("duplicated RMK combos")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_layers() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)
manifest = pc.load_toml(Path("tools/porting_coverage_manifest.toml"))

def source(extra):
    return f'''
/ {{
    keymap {{
        compatible = "zmk,keymap";
        DEFAULT_LAYER {{ bindings = <&kp A>; }};
        SECONDARY_LAYER {{ bindings = <&kp A>; }};
        TERTIARY_LAYER {{ bindings = <&kp A>; }};
        SYSTEM_LAYER {{ bindings = <&kp A>; }};
        {extra}
    }};
}};
'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

print(json.dumps({
    "extra": pack(pc.check_zmk_keymap_layer_inventory(
        manifest,
        source('EXTRA_LAYER { bindings = <&kp A>; };'),
    )),
    "nested_non_layer": pack(pc.check_zmk_keymap_layer_inventory(
        manifest,
        source('metadata { child { bindings = <&kp A>; }; };'),
    )),
}))
"#,
    );

    assert!(
        output.status.success(),
        "layer inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let layer_inventory = &parsed["extra"][0];
    let expected_layers = keyboard_toml()["layout"]["layers"].as_integer().unwrap();
    assert_eq!(layer_inventory["kind"], "zmk_source_inventory");
    assert_eq!(layer_inventory["passed"].as_i64(), Some(expected_layers));
    assert_eq!(layer_inventory["total"].as_i64(), Some(expected_layers + 1));
    assert_eq!(layer_inventory["ok"], false);
    assert!(
        layer_inventory["message"]
            .as_str()
            .unwrap()
            .contains("EXTRA_LAYER")
    );

    let nested_non_layer = &parsed["nested_non_layer"][0];
    assert_eq!(nested_non_layer["kind"], "zmk_source_inventory");
    assert_eq!(nested_non_layer["passed"].as_i64(), Some(expected_layers));
    assert_eq!(nested_non_layer["total"].as_i64(), Some(expected_layers));
    assert_eq!(nested_non_layer["ok"], true);
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_behaviors() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)
manifest = pc.load_toml(Path("tools/porting_coverage_manifest.toml"))

def source(extra):
    return f'''
/ {{
    behaviors {{
        {extra}
    }};
}};
'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

print(json.dumps({
    "reordered": pack(pc.check_zmk_behavior_inventory(
        manifest,
        source('''
        tap_dance_layer_1and2: tap_dance_layer_1and2 { compatible = "zmk,behavior-tap-dance"; };
        mt2: mod_tap2 { compatible = "zmk,behavior-hold-tap"; label = "brace { in string }"; };
        zip_dyn_scale_set: zip_dyn_scale_set { compatible = "zmk,behavior-zip-dynamic-scale-set"; };
        zip_dyn_scale: zip_dyn_scale { compatible = "zmk,behavior-zip-dynamic-scale"; };
        '''),
    )),
    "extra": pack(pc.check_zmk_behavior_inventory(
        manifest,
        source('''
        zip_dyn_scale: zip_dyn_scale { compatible = "zmk,behavior-zip-dynamic-scale"; };
        zip_dyn_scale_set: zip_dyn_scale_set { compatible = "zmk,behavior-zip-dynamic-scale-set"; };
        mt2: mod_tap2 { compatible = "zmk,behavior-hold-tap"; };
        tap_dance_layer_1and2: tap_dance_layer_1and2 { compatible = "zmk,behavior-tap-dance"; };
        new_hold: new_hold { compatible = "zmk,behavior-hold-tap"; };
        '''),
    )),
    "nested_non_behavior": pack(pc.check_zmk_behavior_inventory(
        manifest,
        source('''
        zip_dyn_scale: zip_dyn_scale { compatible = "zmk,behavior-zip-dynamic-scale"; };
        zip_dyn_scale_set: zip_dyn_scale_set { compatible = "zmk,behavior-zip-dynamic-scale-set"; };
        mt2: mod_tap2 { compatible = "zmk,behavior-hold-tap"; };
        tap_dance_layer_1and2: tap_dance_layer_1and2 { compatible = "zmk,behavior-tap-dance"; };
        metadata { child { compatible = "zmk,behavior-hold-tap"; }; };
        '''),
    )),
}))
"#,
    );

    assert!(
        output.status.success(),
        "behavior inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected_behavior_nodes =
        porting_coverage_manifest_toml()["source_inventory"]["behavior_nodes"]
            .as_array()
            .unwrap()
            .len() as i64;

    let reordered = &parsed["reordered"][0];
    assert_eq!(reordered["kind"], "zmk_source_inventory");
    assert_eq!(reordered["passed"].as_i64(), Some(expected_behavior_nodes));
    assert_eq!(reordered["total"].as_i64(), Some(expected_behavior_nodes));
    assert_eq!(reordered["ok"], true);

    let behavior_inventory = &parsed["extra"][0];
    assert_eq!(behavior_inventory["kind"], "zmk_source_inventory");
    assert_eq!(
        behavior_inventory["passed"].as_i64(),
        Some(expected_behavior_nodes)
    );
    assert_eq!(
        behavior_inventory["total"].as_i64(),
        Some(expected_behavior_nodes + 1)
    );
    assert_eq!(behavior_inventory["ok"], false);
    assert!(
        behavior_inventory["message"]
            .as_str()
            .unwrap()
            .contains("new_hold")
    );

    let nested_non_behavior = &parsed["nested_non_behavior"][0];
    assert_eq!(nested_non_behavior["kind"], "zmk_source_inventory");
    assert_eq!(
        nested_non_behavior["passed"].as_i64(),
        Some(expected_behavior_nodes)
    );
    assert_eq!(
        nested_non_behavior["total"].as_i64(),
        Some(expected_behavior_nodes + 1)
    );
    assert_eq!(nested_non_behavior["ok"], false);
    assert!(
        nested_non_behavior["message"]
            .as_str()
            .unwrap()
            .contains("metadata")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_behavior_properties() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "behavior_properties": [{
            "source_file": "lalapadgen2.keymap",
            "source_block": "tap_dance_layer_1and2",
            "expected": [
                'compatible="zmk,behavior-tap-dance"',
                'label="td_mo12"',
                '#binding-cells=<0>',
                'bindings=<&mo 1>, <&mo 2>',
            ],
        }],
    },
}

def source(extra=""):
    return f'''
/ {{
    behaviors {{
        tap_dance_layer_1and2: tap_dance_layer_1and2 {{
            compatible = "zmk,behavior-tap-dance";
            label = "td_mo12";
            #binding-cells = <0>;
            bindings = <&mo 1>, <&mo 2>;
            {extra}
        }};
    }};
}};
'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    fixture = root / "lalapadgen2.keymap"
    fixture.write_text(source())
    ok = pack(pc.check_zmk_behavior_property_inventory(manifest, root))
    fixture.write_text(source('quick-tap-ms = <200>;'))
    changed = pack(pc.check_zmk_behavior_property_inventory(manifest, root))
    fixture.write_text('/ { behaviors { other: other { compatible = "zmk,behavior-tap-dance"; }; }; };')
    missing_block = pack(pc.check_zmk_behavior_property_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_behavior_property_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_block": missing_block,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "behavior property inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(4));
    assert_eq!(ok_inventory["total"].as_i64(), Some(4));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(4));
    assert_eq!(changed_inventory["total"].as_i64(), Some(5));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("quick-tap-ms=<200>")
    );

    let missing_block_inventory = &parsed["missing_block"][0];
    assert_eq!(missing_block_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_block_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_block_inventory["total"].as_i64(), Some(4));
    assert_eq!(missing_block_inventory["ok"], false);
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid behavior property source")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(4));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing behavior property source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_combo_properties() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "combo_properties": [{
            "source_file": "lalapadgen2.keymap",
            "source_block": "COMBO_TAB",
            "expected": [
                'bindings=<&kp TAB>',
                'key-positions=<10 11>',
            ],
        }],
    },
}

def source(extra=""):
    return f'''
/ {{
    combos {{
        compatible = "zmk,combos";
        COMBO_TAB {{
            bindings = <&kp TAB>;
            key-positions = <10 11>;
            {extra}
        }};
    }};
}};
'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    fixture = root / "lalapadgen2.keymap"
    fixture.write_text(source())
    ok = pack(pc.check_zmk_combo_property_inventory(manifest, root))
    fixture.write_text(source('layers = <0>;'))
    changed = pack(pc.check_zmk_combo_property_inventory(manifest, root))
    fixture.write_text('/ { combos { compatible = "zmk,combos"; OTHER { bindings = <&kp TAB>; }; }; };')
    missing_block = pack(pc.check_zmk_combo_property_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_combo_property_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_block": missing_block,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "combo property inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(2));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("layers=<0>")
    );

    let missing_block_inventory = &parsed["missing_block"][0];
    assert_eq!(missing_block_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_block_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_block_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_block_inventory["ok"], false);
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid combo property source")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing combo property source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_dts_status_nodes() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)
manifest = {
    "source_inventory": {
        "dts_status_files": [{
            "source_file": "fixture.overlay",
            "expected": ["&xiao_i2c:okay", "iqs9151:okay"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "fixture.overlay"
    fixture.write_text('''
    &xiao_i2c {
        status = "okay";
        label = "brace { in string }";
        iqs9151: iqs9151@56 {
            status = "okay";
        };
    };
    ''')
    ok = pack(pc.check_zmk_dts_status_inventory(manifest, root))

    fixture.write_text('''
    &xiao_i2c {
        status = "okay";
        iqs9151: iqs9151@56 {
            status = "okay";
        };
        new_listener: listener {
            status = "okay";
        };
    };
    ''')
    extra = pack(pc.check_zmk_dts_status_inventory(manifest, root))

print(json.dumps({"ok": ok, "extra": extra}))
"#,
    );

    assert!(
        output.status.success(),
        "DTS status inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let extra_inventory = &parsed["extra"][0];
    assert_eq!(extra_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_inventory["passed"].as_i64(), Some(2));
    assert_eq!(extra_inventory["total"].as_i64(), Some(3));
    assert_eq!(extra_inventory["ok"], false);
    assert!(
        extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains("new_listener:okay")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_source_files() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "source_files": [
            "lalapadgen2.keymap",
            "boards/shields/lalapadgen2/lalapadgen2.dtsi",
        ],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    shield = root / "boards/shields/lalapadgen2"
    shield.mkdir(parents=True)
    (root / "lalapadgen2.keymap").write_text("")
    (shield / "lalapadgen2.dtsi").write_text("")
    (shield / ".editor.swp").write_text("")
    (shield / "notes.txt").write_text("")
    ok = pack(pc.check_zmk_source_file_inventory(manifest, root))

    (shield / "lalapadgen2_new.overlay").write_text("")
    extra = pack(pc.check_zmk_source_file_inventory(manifest, root))
    (shield / "lalapadgen2.dtsi").unlink()
    missing = pack(pc.check_zmk_source_file_inventory(manifest, root))

print(json.dumps({"ok": ok, "extra": extra, "missing": missing}))
"#,
    );

    assert!(
        output.status.success(),
        "source file inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let extra_inventory = &parsed["extra"][0];
    assert_eq!(extra_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_inventory["passed"].as_i64(), Some(2));
    assert_eq!(extra_inventory["total"].as_i64(), Some(3));
    assert_eq!(extra_inventory["ok"], false);
    assert!(
        extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains("lalapadgen2_new.overlay")
    );

    let missing_inventory = &parsed["missing"][0];
    assert_eq!(missing_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_inventory["passed"].as_i64(), Some(1));
    assert_eq!(missing_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_inventory["ok"], false);
    assert!(
        missing_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing source files")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_repo_files() {
    let output = run_python(
        r#"
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "repo_files": [
            ".github/workflows/build.yml",
            "build.yaml",
            "config/lalapadgen2.keymap",
        ],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    config = root / "config"
    workflow = root / ".github/workflows"
    workflow.mkdir(parents=True)
    config.mkdir()
    (workflow / "build.yml").write_text("")
    (root / "build.yaml").write_text("")
    (config / "lalapadgen2.keymap").write_text("")
    (root / ".git").mkdir()
    (root / ".git" / "HEAD").write_text("")
    (root / ".west").mkdir()
    (root / ".west" / "config").write_text("")
    (root / ".DS_Store").write_text("")
    ok = pack(pc.check_zmk_repo_file_inventory(manifest, config))

    (root / "README.md").write_text("")
    extra = pack(pc.check_zmk_repo_file_inventory(manifest, config))
    (root / "build.yaml").unlink()
    missing = pack(pc.check_zmk_repo_file_inventory(manifest, config))

with tempfile.TemporaryDirectory() as tempdir:
    git_root = Path(tempdir)
    config = git_root / "config"
    workflow = git_root / ".github/workflows"
    workflow.mkdir(parents=True)
    config.mkdir()
    (workflow / "build.yml").write_text("")
    (git_root / "build.yaml").write_text("")
    (config / "lalapadgen2.keymap").write_text("")
    subprocess.run(["git", "init"], cwd=git_root, check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["git", "add", "."], cwd=git_root, check=True, stdout=subprocess.DEVNULL)
    (git_root / "build.yaml").unlink()
    (git_root / "README.md").write_text("")
    dirty_git = pack(pc.check_zmk_repo_file_inventory(manifest, config))

print(json.dumps({"ok": ok, "extra": extra, "missing": missing, "dirty_git": dirty_git}))
"#,
    );

    assert!(
        output.status.success(),
        "repo file inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);

    let extra_inventory = &parsed["extra"][0];
    assert_eq!(extra_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_inventory["passed"].as_i64(), Some(3));
    assert_eq!(extra_inventory["total"].as_i64(), Some(4));
    assert_eq!(extra_inventory["ok"], false);
    assert!(
        extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains("README.md")
    );
    assert!(
        !extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains(".git/HEAD")
    );
    assert!(
        !extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains(".west/config")
    );
    assert!(
        !extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains(".DS_Store")
    );

    let missing_inventory = &parsed["missing"][0];
    assert_eq!(missing_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_inventory["passed"].as_i64(), Some(2));
    assert_eq!(missing_inventory["total"].as_i64(), Some(4));
    assert_eq!(missing_inventory["ok"], false);
    assert!(
        missing_inventory["message"]
            .as_str()
            .unwrap()
            .contains("build.yaml")
    );

    let dirty_git_inventory = &parsed["dirty_git"][0];
    assert_eq!(dirty_git_inventory["kind"], "zmk_inventory");
    assert_eq!(dirty_git_inventory["passed"].as_i64(), Some(2));
    assert_eq!(dirty_git_inventory["total"].as_i64(), Some(4));
    assert_eq!(dirty_git_inventory["ok"], false);
    assert!(
        dirty_git_inventory["message"]
            .as_str()
            .unwrap()
            .contains("build.yaml")
    );
    assert!(
        dirty_git_inventory["message"]
            .as_str()
            .unwrap()
            .contains("README.md")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_includes() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "include_files": [{
            "source_file": "fixture.keymap",
            "expected": ["<behaviors.dtsi>", "<dt-bindings/zmk/keys.h>"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "fixture.keymap"
    fixture.write_text('''
    /* #include <disabled/commented-out.dtsi> */
    #include <behaviors.dtsi>
    #include <dt-bindings/zmk/keys.h>
    / {};
    ''')
    ok = pack(pc.check_zmk_include_inventory(manifest, root))

    fixture.write_text('''
    #include <behaviors.dtsi>
    #include <dt-bindings/zmk/pointing.h>
    #include <dt-bindings/zmk/keys.h>
    / {};
    ''')
    extra = pack(pc.check_zmk_include_inventory(manifest, root))

    fixture.write_text('''
    #include <behaviors.dtsi>
    / {};
    ''')
    missing = pack(pc.check_zmk_include_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_include_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "extra": extra,
    "missing": missing,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "include inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let extra_inventory = &parsed["extra"][0];
    assert_eq!(extra_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_inventory["passed"].as_i64(), Some(1));
    assert_eq!(extra_inventory["total"].as_i64(), Some(3));
    assert_eq!(extra_inventory["ok"], false);
    assert!(
        extra_inventory["message"]
            .as_str()
            .unwrap()
            .contains("dt-bindings/zmk/pointing.h")
    );

    let missing_inventory = &parsed["missing"][0];
    assert_eq!(missing_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_inventory["passed"].as_i64(), Some(1));
    assert_eq!(missing_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_inventory["ok"], false);
    assert!(
        missing_inventory["message"]
            .as_str()
            .unwrap()
            .contains("got None")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing include source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_kconfig_entries() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "kconfig_entries": [{
            "source_file": "Kconfig.defconfig",
            "expected": ["ZMK_SPLIT:default y", "BT_MAX_CONN:default 5"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "Kconfig.defconfig"
    fixture.write_text('''
    config ZMK_SPLIT
        default y

    config BT_MAX_CONN
        default 5
    ''')
    ok = pack(pc.check_zmk_kconfig_entry_inventory(manifest, root))

    fixture.write_text('''
    config ZMK_SPLIT
        default y

    config BT_MAX_CONN
        default 6

    config NEW_SOURCE_SETTING
        default y
    ''')
    changed = pack(pc.check_zmk_kconfig_entry_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_kconfig_entry_inventory(manifest, root))

print(json.dumps({"ok": ok, "changed": changed, "missing_file": missing_file}))
"#,
    );

    assert!(
        output.status.success(),
        "Kconfig entry inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("NEW_SOURCE_SETTING")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing Kconfig source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_kconfig_lines() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "kconfig_lines": [{
            "source_file": "lalapadgen2.conf",
            "expected": [
                "CONFIG_BT_BAS=y",
                "CONFIG_ZMK_STUDIO_LOCKING=n",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.conf"
    fixture.write_text('''
    # BLE
    CONFIG_BT_BAS=y
    #CONFIG_ZMK_USB_LOGGING=y
    CONFIG_ZMK_STUDIO_LOCKING=n
    ''')
    ok = pack(pc.check_zmk_kconfig_line_inventory(manifest, root))

    fixture.write_text('''
    CONFIG_BT_BAS=y
    CONFIG_ZMK_STUDIO_LOCKING=y
    CONFIG_NEW_SOURCE_SETTING=42
    ''')
    changed = pack(pc.check_zmk_kconfig_line_inventory(manifest, root))

    fixture.write_text('''
    CONFIG_ZMK_STUDIO_LOCKING=n
    CONFIG_BT_BAS=y
    ''')
    reordered = pack(pc.check_zmk_kconfig_line_inventory(manifest, root))

    fixture.write_text('''
    CONFIG_BT_BAS=y
    #CONFIG_ZMK_STUDIO_LOCKING=n
    ''')
    commented_out = pack(pc.check_zmk_kconfig_line_inventory(manifest, root))

    fixture.unlink()
    missing_file = pack(pc.check_zmk_kconfig_line_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "reordered": reordered,
    "commented_out": commented_out,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "Kconfig line inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("CONFIG_NEW_SOURCE_SETTING=42")
    );

    let reordered_inventory = &parsed["reordered"][0];
    assert_eq!(reordered_inventory["kind"], "zmk_inventory");
    assert_eq!(reordered_inventory["passed"].as_i64(), Some(0));
    assert_eq!(reordered_inventory["total"].as_i64(), Some(2));
    assert_eq!(reordered_inventory["ok"], false);
    assert!(
        reordered_inventory["message"]
            .as_str()
            .unwrap()
            .contains("CONFIG_ZMK_STUDIO_LOCKING=n")
    );

    let commented_out_inventory = &parsed["commented_out"][0];
    assert_eq!(commented_out_inventory["kind"], "zmk_inventory");
    assert_eq!(commented_out_inventory["passed"].as_i64(), Some(1));
    assert_eq!(commented_out_inventory["total"].as_i64(), Some(2));
    assert_eq!(commented_out_inventory["ok"], false);
    assert!(
        commented_out_inventory["message"]
            .as_str()
            .unwrap()
            .contains("got None")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing ZMK Kconfig source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_disabled_zmk_kconfig_lines() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "disabled_kconfig_lines": [{
            "source_file": "lalapadgen2.conf",
            "expected": [
                "CONFIG_ZMK_USB_LOGGING=y",
                "CONFIG_INPUT_LOG_LEVEL_DBG=y",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.conf"
    fixture.write_text('''
    CONFIG_BT_BAS=y
    #CONFIG_ZMK_USB_LOGGING=y
    # CONFIG_INPUT_LOG_LEVEL_DBG=y
    ''')
    ok = pack(pc.check_zmk_disabled_kconfig_line_inventory(manifest, root))

    fixture.write_text('''
    #CONFIG_ZMK_USB_LOGGING=y
    #CONFIG_INPUT_LOG_LEVEL_DBG=n
    #CONFIG_NEW_DISABLED_SETTING=42
    ''')
    changed = pack(pc.check_zmk_disabled_kconfig_line_inventory(manifest, root))

    fixture.write_text('''
    CONFIG_ZMK_USB_LOGGING=y
    #CONFIG_INPUT_LOG_LEVEL_DBG=y
    ''')
    enabled = pack(pc.check_zmk_disabled_kconfig_line_inventory(manifest, root))

    fixture.unlink()
    missing_file = pack(pc.check_zmk_disabled_kconfig_line_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "enabled": enabled,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "disabled Kconfig line inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("CONFIG_NEW_DISABLED_SETTING=42")
    );

    let enabled_inventory = &parsed["enabled"][0];
    assert_eq!(enabled_inventory["kind"], "zmk_inventory");
    assert_eq!(enabled_inventory["passed"].as_i64(), Some(0));
    assert_eq!(enabled_inventory["total"].as_i64(), Some(2));
    assert_eq!(enabled_inventory["ok"], false);
    assert!(
        enabled_inventory["message"]
            .as_str()
            .unwrap()
            .contains("CONFIG_INPUT_LOG_LEVEL_DBG=y")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing ZMK Kconfig source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_define_entries() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "define_entries": [{
            "source_file": "lalapadgen2.dtsi",
            "prefix": "POS_TP_",
            "expected": ["POS_TP_LCLK_L=52", "POS_TP_RCLK_L=53"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    #define POS_TP_LCLK_L 52
    #define POS_TP_RCLK_L 53
    #define OTHER_DEFINE 99
    ''')
    ok = pack(pc.check_zmk_define_entry_inventory(manifest, root))

    fixture.write_text('''
    #define POS_TP_LCLK_L 52
    #define POS_TP_RCLK_L 54
    #define POS_TP_NEW 68
    ''')
    changed = pack(pc.check_zmk_define_entry_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_define_entry_inventory(manifest, root))

print(json.dumps({"ok": ok, "changed": changed, "missing_file": missing_file}))
"#,
    );

    assert!(
        output.status.success(),
        "define entry inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("POS_TP_NEW")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing define source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_dts_aliases() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "dts_aliases": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "aliases",
            "expected": ["led-red=&led0", "led-green=&led1", "led-blue=&led2"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    / {
        aliases {
            led-red = &led0;
            led-green = &led1;
            led-blue = &led2;
        };
        nested {
            aliases {
                led-red = &other;
            };
        };
    };
    ''')
    ok = pack(pc.check_zmk_dts_alias_inventory(manifest, root))

    fixture.write_text('''
    / {
        aliases {
            led-red = &led1;
            led-green = &led0;
            led-blue = &led2;
            led-white = &led3;
        };
    };
    ''')
    changed = pack(pc.check_zmk_dts_alias_inventory(manifest, root))

    fixture.write_text('/ { chosen { zmk,physical-layout = &layout; }; };')
    missing_block = pack(pc.check_zmk_dts_alias_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_dts_alias_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_block": missing_block,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "DTS alias inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(4));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("led-white=&led3")
    );

    let missing_block_inventory = &parsed["missing_block"][0];
    assert_eq!(missing_block_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_block_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_block_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_block_inventory["ok"], false);
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("block 'aliases' not found")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing DTS alias source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_dts_nodes() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

root_manifest = {
    "source_inventory": {
        "dts_node_inventories": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "/",
            "expected": ["chosen", "kscan0"],
        }],
    },
}
overlay_manifest = {
    "source_inventory": {
        "dts_node_inventories": [{
            "source_file": "right.overlay",
            "source_block": "__top__",
            "expected": ["&default_transform", "&kscan0"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    overlay = root / "right.overlay"
    fixture.write_text('''
    / {
        chosen {
            zmk,physical-layout = &layout;
        };
        kscan0: kscan {
            nested_child {
                status = "okay";
            };
        };
    };
    &xiao_i2c {
        iqs9151: iqs9151@56 {
            status = "okay";
        };
    };
    ''')
    ok = pack(pc.check_zmk_dts_node_inventory(root_manifest, root))

    fixture.write_text('''
    / {
        chosen {};
        new_node {};
        kscan0: kscan {};
    };
    ''')
    changed = pack(pc.check_zmk_dts_node_inventory(root_manifest, root))

    overlay.write_text('''
    &default_transform {
        col-offset = <6>;
    };
    &kscan0 {
        nested_child {};
    };
    ''')
    overlay_ok = pack(pc.check_zmk_dts_node_inventory(overlay_manifest, root))

    overlay.write_text('''
    &default_transform {};
    &trackpad_listener_R {};
    &kscan0 {};
    ''')
    overlay_changed = pack(pc.check_zmk_dts_node_inventory(overlay_manifest, root))

    fixture.write_text('&kscan0 {};')
    missing_root = pack(pc.check_zmk_dts_node_inventory(root_manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_dts_node_inventory(root_manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "overlay_ok": overlay_ok,
    "overlay_changed": overlay_changed,
    "missing_root": missing_root,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "DTS node inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(
        ok_inventory["id"],
        "zmk_source.dts_nodes.lalapadgen2.dtsi./"
    );
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("new_node")
    );

    let overlay_inventory = &parsed["overlay_ok"][0];
    assert_eq!(
        overlay_inventory["id"],
        "zmk_source.dts_nodes.right.overlay.__top__"
    );
    assert_eq!(overlay_inventory["kind"], "zmk_inventory");
    assert_eq!(overlay_inventory["passed"].as_i64(), Some(2));
    assert_eq!(overlay_inventory["total"].as_i64(), Some(2));
    assert_eq!(overlay_inventory["ok"], true);

    let overlay_changed_inventory = &parsed["overlay_changed"][0];
    assert_eq!(overlay_changed_inventory["kind"], "zmk_inventory");
    assert_eq!(overlay_changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(overlay_changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(overlay_changed_inventory["ok"], false);
    assert!(
        overlay_changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("&trackpad_listener_R")
    );

    let missing_root_inventory = &parsed["missing_root"][0];
    assert_eq!(missing_root_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_root_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_root_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_root_inventory["ok"], false);
    assert!(
        missing_root_inventory["message"]
            .as_str()
            .unwrap()
            .contains("root block '/' not found")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing DTS node source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_physical_layout_attrs() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "physical_layout_attrs": [{
            "source_file": "lalapadgen2-layouts.dtsi",
            "source_block": "lalapadgen2_physical_layout",
            "expected": ["100,100,0,0,0,0,0", "50,50,400,450,0,0,0"],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2-layouts.dtsi"
    fixture.write_text('''
    / {
        lalapadgen2_physical_layout: lalapadgen2_physical_layout {
            keys = <&key_physical_attrs 100 100 0 0 0 0 0>,
                   <&key_physical_attrs 50 50 400 450 0 0 0>;
        };
    };
    ''')
    ok = pack(pc.check_zmk_physical_layout_attr_inventory(manifest, root))

    fixture.write_text('''
    / {
        lalapadgen2_physical_layout: lalapadgen2_physical_layout {
            keys = <&key_physical_attrs 100 100 0 0 0 0 0>,
                   <&key_physical_attrs 50 50 450 450 0 0 0>,
                   <&key_physical_attrs 100 100 1000 600 0 0 0>;
        };
    };
    ''')
    changed = pack(pc.check_zmk_physical_layout_attr_inventory(manifest, root))
    fixture.write_text('''
    / {
        other_physical_layout: other_physical_layout {
            keys = <&key_physical_attrs 100 100 0 0 0 0 0>;
        };
    };
    ''')
    missing_block = pack(pc.check_zmk_physical_layout_attr_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_physical_layout_attr_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_block": missing_block,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "physical layout attr inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("100,100,1000,600,0,0,0")
    );

    let missing_block_inventory = &parsed["missing_block"][0];
    assert_eq!(missing_block_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_block_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_block_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_block_inventory["ok"], false);
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid physical layout source")
    );
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("block 'lalapadgen2_physical_layout' not found")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing physical layout source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_input_behavior_bindings() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "input_behavior_bindings": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "trackpad_key_behaviors_R",
            "expected": [
                "INPUT_BTN_0:&tp_to_pos POS_TP_LCLK_R",
                "INPUT_BTN_1:&tp_to_pos POS_TP_RCLK_R",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    trackpad_key_behaviors_R: trackpad_key_behaviors_R {
        codes = <INPUT_BTN_0 INPUT_BTN_1>;
        bindings = <&tp_to_pos POS_TP_LCLK_R &tp_to_pos POS_TP_RCLK_R>;
    };
    ''')
    ok = pack(pc.check_zmk_input_behavior_binding_inventory(manifest, root))

    fixture.write_text('''
    trackpad_key_behaviors_R: trackpad_key_behaviors_R {
        codes = <INPUT_BTN_0 INPUT_BTN_1 INPUT_BTN_2>;
        bindings = <&tp_to_pos POS_TP_LCLK_R &tp_to_pos POS_TP_PINCH_R &tp_to_pos POS_TP_NEW_R>;
    };
    ''')
    changed = pack(pc.check_zmk_input_behavior_binding_inventory(manifest, root))

    fixture.write_text('''
    trackpad_key_behaviors_R: trackpad_key_behaviors_R {
        codes = <INPUT_BTN_0 INPUT_BTN_1>;
        bindings = <&tp_to_pos POS_TP_LCLK_R>;
    };
    ''')
    mismatched_lengths = pack(pc.check_zmk_input_behavior_binding_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_input_behavior_binding_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "mismatched_lengths": mismatched_lengths,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "input behavior binding inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("POS_TP_NEW_R")
    );

    let mismatched_lengths_inventory = &parsed["mismatched_lengths"][0];
    assert_eq!(mismatched_lengths_inventory["kind"], "zmk_inventory");
    assert_eq!(mismatched_lengths_inventory["passed"].as_i64(), Some(0));
    assert_eq!(mismatched_lengths_inventory["total"].as_i64(), Some(2));
    assert_eq!(mismatched_lengths_inventory["ok"], false);
    assert!(
        mismatched_lengths_inventory["message"]
            .as_str()
            .unwrap()
            .contains("codes/bindings length mismatch")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing input behavior source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_input_listeners() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "input_listeners": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "trackpad_listener_R",
            "expected": [
                "device=&trackpad_split_R",
                "input-processors:&trackpad_key_behaviors_R",
                "input-processors:&zip_xy_scaler 1 5",
                "lowspeed.layers=1,2",
                "lowspeed.input-processors:&trackpad_key_behaviors_R",
                "lowspeed.input-processors:&zip_xy_scaler 1 15",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    trackpad_listener_R: trackpad_listener_R {
        device = <&trackpad_split_R>;
        input-processors = <&trackpad_key_behaviors_R>,
                           <&zip_xy_scaler 1 5>;
        lowspeedmode {
            layers = <1>,<2>;
            input-processors = <&trackpad_key_behaviors_R>,
                               <&zip_xy_scaler 1 15>;
        };
    };
    ''')
    ok = pack(pc.check_zmk_input_listener_inventory(manifest, root))

    fixture.write_text('''
    trackpad_listener_R: trackpad_listener_R {
        device = <&iqs9151>;
        input-processors = <&trackpad_key_behaviors_R>,
                           <&zip_xy_scaler 1 6>,
                           <&zip_dynamic_xy_scaler>;
        lowspeedmode {
            layers = <1>,<3>;
            input-processors = <&trackpad_key_behaviors_R>,
                               <&zip_xy_scaler 1 15>;
        };
    };
    ''')
    changed = pack(pc.check_zmk_input_listener_inventory(manifest, root))

    fixture.write_text('''
    trackpad_listener_R: trackpad_listener_R {
        device = <&trackpad_split_R>;
        lowspeedmode {
            layers = <1>,<2>;
        };
    };
    ''')
    missing_processors = pack(pc.check_zmk_input_listener_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_input_listener_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_processors": missing_processors,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "input listener inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(6));
    assert_eq!(ok_inventory["total"].as_i64(), Some(6));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(7));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("&zip_xy_scaler 1 6")
    );
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("lowspeed.layers=1,3")
    );

    let missing_processors_inventory = &parsed["missing_processors"][0];
    assert_eq!(missing_processors_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_processors_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_processors_inventory["total"].as_i64(), Some(6));
    assert_eq!(missing_processors_inventory["ok"], false);
    assert!(
        missing_processors_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid input listener source")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(6));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing input listener source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_dts_properties() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "dts_properties": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "zip_dynamic_xy_scaler",
            "expected": [
                "compatible=\"zmk,input-processor-dynamic-scaler\"",
                "type=<INPUT_EV_REL>",
                "codes=<INPUT_REL_X INPUT_REL_Y>",
                "scale-group=<ZDS_XY>",
                "track-remainders",
                "device=<&iqs9151>",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    zip_dynamic_xy_scaler: zip_dynamic_xy_scaler {
        compatible = "zmk,input-processor-dynamic-scaler";
        type = <INPUT_EV_REL>;
        codes = <INPUT_REL_X INPUT_REL_Y>;
        scale-group = <ZDS_XY>;
        track-remainders;
        device = <&iqs9151>;
        child { type = <SHOULD_NOT_BE_COLLECTED>; };
    };
    ''')
    ok = pack(pc.check_zmk_dts_property_inventory(manifest, root))

    fixture.write_text('''
    zip_dynamic_xy_scaler: zip_dynamic_xy_scaler {
        compatible = "zmk,input-processor-dynamic-scaler";
        type = <INPUT_EV_REL>;
        codes = <INPUT_REL_X INPUT_REL_WHEEL>;
        scale-group = <ZDS_SC>;
        track-remainders;
        extra-prop;
        device = <&trackpad_split_R>;
    };
    ''')
    changed = pack(pc.check_zmk_dts_property_inventory(manifest, root))

    fixture.write_text('''
    other_node: other_node {
        compatible = "zmk,input-processor-dynamic-scaler";
    };
    ''')
    missing_block = pack(pc.check_zmk_dts_property_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_dts_property_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_block": missing_block,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "DTS property inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(
        ok_inventory["id"],
        "zmk_source.dts_properties.lalapadgen2.dtsi.zip_dynamic_xy_scaler"
    );
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(6));
    assert_eq!(ok_inventory["total"].as_i64(), Some(6));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(3));
    assert_eq!(changed_inventory["total"].as_i64(), Some(7));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("extra-prop")
    );

    let missing_block_inventory = &parsed["missing_block"][0];
    assert_eq!(missing_block_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_block_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_block_inventory["total"].as_i64(), Some(6));
    assert_eq!(missing_block_inventory["ok"], false);
    assert!(
        missing_block_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid DTS property source")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(
        missing_file_inventory["id"],
        "zmk_source.dts_properties.lalapadgen2.dtsi.zip_dynamic_xy_scaler"
    );
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(6));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing DTS property source file")
    );
}

#[test]
fn porting_coverage_rejects_rust_const_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "rust_const_values": [
        {
            "id": "hex_const",
            "target_file": "firmware.rs",
            "target_const": "ADDR_PRODUCT_NUMBER",
            "expected": 4096,
        },
        {
            "id": "bool_const",
            "target_file": "firmware.rs",
            "target_const": "DEFAULT_CURSOR_INERTIA_ENABLED",
            "expected": True,
        },
        {
            "id": "expression_const",
            "target_file": "firmware.rs",
            "target_const": "INFO_SHOW_RESET",
            "expected": "1 << 7",
        },
        {
            "id": "source_backed_const",
            "source_file": "source.conf",
            "source_key": "CONFIG_INPUT_IQS9151_RESOLUTION_X",
            "target_file": "firmware.rs",
            "target_const": "DEFAULT_X_RESOLUTION",
            "expected": 2457,
        },
        {
            "id": "source_backed_bool_const",
            "source_file": "source.conf",
            "source_key": "CONFIG_INPUT_IQS9151_1F_TAP_ENABLE",
            "source_expected": "y",
            "target_file": "firmware.rs",
            "target_const": "DEFAULT_ONE_FINGER_TAP_ENABLED",
            "expected": True,
        },
        {
            "id": "missing_const",
            "target_file": "firmware.rs",
            "target_const": "MISSING_CONST",
            "expected": 1,
        },
        {
            "id": "missing_source",
            "source_file": "missing.conf",
            "source_key": "CONFIG_INPUT_IQS9151_RESOLUTION_X",
            "target_file": "firmware.rs",
            "target_const": "DEFAULT_X_RESOLUTION",
            "expected": 2457,
        },
        {
            "id": "missing_target_file",
            "target_file": "missing.rs",
            "target_const": "DEFAULT_X_RESOLUTION",
            "expected": 2457,
        },
    ],
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    (root / "source.conf").write_text("CONFIG_INPUT_IQS9151_RESOLUTION_X=2457\nCONFIG_INPUT_IQS9151_1F_TAP_ENABLE=y\n")
    firmware = root / "firmware.rs"
    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
    const DEFAULT_CURSOR_INERTIA_ENABLED: bool = true;
    pub const INFO_SHOW_RESET: u16 = 1 << 7;
    pub const DEFAULT_X_RESOLUTION: u16 = 2457;
    pub const DEFAULT_ONE_FINGER_TAP_ENABLED: bool = true;
    ''')
    ok = pack(pc.check_rust_const_values(manifest, root, root))

    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1002;
    const DEFAULT_CURSOR_INERTIA_ENABLED: bool = false;
    pub const INFO_SHOW_RESET: u16 = 1 << 6;
    pub const DEFAULT_X_RESOLUTION: u16 = 2000;
    pub const DEFAULT_ONE_FINGER_TAP_ENABLED: bool = false;
    ''')
    (root / "source.conf").write_text("CONFIG_INPUT_IQS9151_RESOLUTION_X=2000\nCONFIG_INPUT_IQS9151_1F_TAP_ENABLE=n\n")
    changed = pack(pc.check_rust_const_values(manifest, root, root))

print(json.dumps({"ok": ok, "changed": changed}))
"#,
    );

    assert!(
        output.status.success(),
        "Rust const inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for id in ["hex_const", "bool_const", "expression_const"] {
        let inventory = parsed["ok"]
            .as_array()
            .unwrap()
            .iter()
            .find(|result| result["id"] == id)
            .unwrap_or_else(|| panic!("missing rust const inventory result for {id}"));
        assert_eq!(inventory["kind"], "rust_const");
        assert_eq!(inventory["passed"].as_i64(), Some(1));
        assert_eq!(inventory["total"].as_i64(), Some(1));
        assert_eq!(inventory["ok"], true);
    }

    let source_backed = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "source_backed_const")
        .expect("missing source-backed rust const result");
    assert_eq!(source_backed["kind"], "rust_const");
    assert_eq!(source_backed["passed"].as_i64(), Some(2));
    assert_eq!(source_backed["total"].as_i64(), Some(2));
    assert_eq!(source_backed["ok"], true);

    let source_backed_bool = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "source_backed_bool_const")
        .expect("missing source-backed bool rust const result");
    assert_eq!(source_backed_bool["kind"], "rust_const");
    assert_eq!(source_backed_bool["passed"].as_i64(), Some(2));
    assert_eq!(source_backed_bool["total"].as_i64(), Some(2));
    assert_eq!(source_backed_bool["ok"], true);

    let missing_const = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_const")
        .expect("missing missing-const result");
    assert_eq!(missing_const["kind"], "rust_const");
    assert_eq!(missing_const["passed"].as_i64(), Some(0));
    assert_eq!(missing_const["total"].as_i64(), Some(1));
    assert_eq!(missing_const["ok"], false);
    assert!(
        missing_const["message"]
            .as_str()
            .unwrap()
            .contains("not found")
    );

    let missing_source = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_source")
        .expect("missing missing-source result");
    assert_eq!(missing_source["kind"], "rust_const");
    assert_eq!(missing_source["passed"].as_i64(), Some(1));
    assert_eq!(missing_source["total"].as_i64(), Some(2));
    assert_eq!(missing_source["ok"], false);
    assert!(
        missing_source["message"]
            .as_str()
            .unwrap()
            .contains("missing source Kconfig file")
    );

    let missing_target_file = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_target_file")
        .expect("missing missing-target-file result");
    assert_eq!(missing_target_file["kind"], "rust_const");
    assert_eq!(missing_target_file["passed"].as_i64(), Some(0));
    assert_eq!(missing_target_file["total"].as_i64(), Some(1));
    assert_eq!(missing_target_file["ok"], false);
    assert!(
        missing_target_file["message"]
            .as_str()
            .unwrap()
            .contains("missing.rs")
    );

    let changed_hex = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "hex_const")
        .expect("missing changed hex const result");
    assert_eq!(changed_hex["kind"], "rust_const");
    assert_eq!(changed_hex["passed"].as_i64(), Some(0));
    assert_eq!(changed_hex["total"].as_i64(), Some(1));
    assert!(changed_hex["message"].as_str().unwrap().contains("4098"));

    let changed_source = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "source_backed_const")
        .expect("missing changed source-backed const result");
    assert_eq!(changed_source["kind"], "rust_const");
    assert_eq!(changed_source["passed"].as_i64(), Some(0));
    assert_eq!(changed_source["total"].as_i64(), Some(2));
    assert!(
        changed_source["message"]
            .as_str()
            .unwrap()
            .contains("source expected")
    );

    let changed_bool_source = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "source_backed_bool_const")
        .expect("missing changed source-backed bool const result");
    assert_eq!(changed_bool_source["kind"], "rust_const");
    assert_eq!(changed_bool_source["passed"].as_i64(), Some(0));
    assert_eq!(changed_bool_source["total"].as_i64(), Some(2));
    assert!(
        changed_bool_source["message"]
            .as_str()
            .unwrap()
            .contains("source expected 'y', got 'n'")
    );
}

#[test]
fn porting_coverage_rejects_rust_const_inventory_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "rust_const_inventories": [
        {
            "id": "register_addresses",
            "target_file": "firmware.rs",
            "name_regex": "ADDR_.*",
            "expected": [
                "ADDR_PRODUCT_NUMBER",
                "ADDR_RELATIVE_X",
                "ADDR_RELATIVE_Y",
            ],
        },
        {
            "id": "missing_file",
            "target_file": "missing.rs",
            "name_regex": "ADDR_.*",
            "expected": ["ADDR_PRODUCT_NUMBER"],
        },
        {
            "id": "bad_regex",
            "target_file": "firmware.rs",
            "name_regex": "[",
            "expected": ["ADDR_PRODUCT_NUMBER"],
        },
    ],
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    firmware = root / "firmware.rs"
    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
    pub const ADDR_RELATIVE_X: u16 = 0x1014;
    pub const ADDR_RELATIVE_Y: u16 = 0x1016;
    pub const DEFAULT_X_RESOLUTION: u16 = 2457;
    ''')
    ok = pack(pc.check_rust_const_inventories(manifest, root))

    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
    pub const ADDR_RELATIVE_Y: u16 = 0x1016;
    pub const ADDR_EXTRA: u16 = 0x9999;
    ''')
    changed = pack(pc.check_rust_const_inventories(manifest, root))

    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
    pub const ADDR_RELATIVE_X: u16 = 0x1014;
    pub const ADDR_RELATIVE_Y: u16 = 0x1016;
    pub const ADDR_EXTRA: u16 = 0x9999;
    ''')
    appended = pack(pc.check_rust_const_inventories(manifest, root))

print(json.dumps({"ok": ok, "changed": changed, "appended": appended}))
"#,
    );

    assert!(
        output.status.success(),
        "Rust const inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "register_addresses")
        .expect("missing ok const inventory result");
    assert_eq!(ok_inventory["kind"], "rust_const_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);

    let missing_file = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_file")
        .expect("missing missing-file inventory result");
    assert_eq!(missing_file["kind"], "rust_const_inventory");
    assert_eq!(missing_file["passed"].as_i64(), Some(0));
    assert_eq!(missing_file["total"].as_i64(), Some(1));
    assert_eq!(missing_file["ok"], false);
    assert!(
        missing_file["message"]
            .as_str()
            .unwrap()
            .contains("missing.rs")
    );

    let bad_regex = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "bad_regex")
        .expect("missing bad-regex inventory result");
    assert_eq!(bad_regex["kind"], "rust_const_inventory");
    assert_eq!(bad_regex["passed"].as_i64(), Some(0));
    assert_eq!(bad_regex["total"].as_i64(), Some(1));
    assert_eq!(bad_regex["ok"], false);

    let changed = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "register_addresses")
        .expect("missing changed const inventory result");
    assert_eq!(changed["kind"], "rust_const_inventory");
    assert_eq!(changed["passed"].as_i64(), Some(1));
    assert_eq!(changed["total"].as_i64(), Some(3));
    assert_eq!(changed["ok"], false);
    assert!(
        changed["message"]
            .as_str()
            .unwrap()
            .contains("expected 'ADDR_RELATIVE_X', got 'ADDR_RELATIVE_Y'")
    );

    let appended = parsed["appended"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "register_addresses")
        .expect("missing appended const inventory result");
    assert_eq!(appended["kind"], "rust_const_inventory");
    assert_eq!(appended["passed"].as_i64(), Some(3));
    assert_eq!(appended["total"].as_i64(), Some(4));
    assert_eq!(appended["ok"], false);
    assert!(
        appended["message"]
            .as_str()
            .unwrap()
            .contains("expected None, got 'ADDR_EXTRA'")
    );
}

#[test]
fn porting_coverage_rejects_iqs9151_register_porting_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

ok_manifest = {
    "source_inventory": {
        "iqs9151_register_addresses": [
            "IQS9151_ADDR_A=1",
            "IQS9151_ADDR_B=2",
            "IQS9151_ADDR_C=3",
        ],
    },
    "iqs9151_register_porting": [
        {"source_const": "IQS9151_ADDR_A", "source_value": 1, "status": "ported", "target_const": "ADDR_A"},
        {"source_const": "IQS9151_ADDR_B", "source_value": 2, "status": "not_ported", "reason": "not needed"},
        {"source_const": "IQS9151_ADDR_C", "source_value": 3, "status": "ported", "target_const": "ADDR_C"},
    ],
}
bad_manifest = {
    "source_inventory": ok_manifest["source_inventory"],
    "iqs9151_register_porting": [
        {"source_const": "IQS9151_ADDR_A", "source_value": 1, "status": "ported", "target_const": "ADDR_A"},
        {"source_const": "IQS9151_ADDR_B", "source_value": 2, "status": "not_ported", "reason": ""},
        {"source_const": "IQS9151_ADDR_D", "source_value": 4, "status": "ported", "target_const": "ADDR_D"},
    ],
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    (root / "src").mkdir()
    (root / "src" / "iqs9151.rs").write_text('''
    pub const ADDR_A: u16 = 1;
    pub const ADDR_C: u16 = 3;
    pub const ADDR_D: u16 = 5;
    ''')
    ok = pack(pc.check_iqs9151_register_porting(ok_manifest, root))
    bad = pack(pc.check_iqs9151_register_porting(bad_manifest, root))

print(json.dumps({"ok": ok, "bad": bad}))
"#,
    );

    assert!(
        output.status.success(),
        "IQS9151 register porting parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_register_address_classification")
        .expect("missing ok register classification result");
    assert_eq!(ok_inventory["kind"], "iqs9151_register_porting");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);
    for result in parsed["ok"].as_array().unwrap() {
        assert_eq!(result["kind"], "iqs9151_register_porting");
        assert_eq!(result["ok"], true);
    }

    let bad_inventory = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_register_address_classification")
        .expect("missing bad register classification result");
    assert_eq!(bad_inventory["passed"].as_i64(), Some(2));
    assert_eq!(bad_inventory["total"].as_i64(), Some(3));
    assert_eq!(bad_inventory["ok"], false);
    assert!(
        bad_inventory["message"]
            .as_str()
            .unwrap()
            .contains("expected 'IQS9151_ADDR_C=3', got 'IQS9151_ADDR_D=4'")
    );

    let missing_reason = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_register_porting.IQS9151_ADDR_B")
        .expect("missing no-reason result");
    assert_eq!(missing_reason["passed"].as_i64(), Some(1));
    assert_eq!(missing_reason["total"].as_i64(), Some(2));
    assert_eq!(missing_reason["ok"], false);
    assert!(
        missing_reason["message"]
            .as_str()
            .unwrap()
            .contains("has no reason")
    );

    let extra_source = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_register_porting.IQS9151_ADDR_D")
        .expect("missing extra-source result");
    assert_eq!(extra_source["passed"].as_i64(), Some(0));
    assert_eq!(extra_source["total"].as_i64(), Some(2));
    assert_eq!(extra_source["ok"], false);
    assert!(
        extra_source["message"]
            .as_str()
            .unwrap()
            .contains("is not in inventory")
    );
    assert!(
        extra_source["message"]
            .as_str()
            .unwrap()
            .contains("ADDR_D expected source value 4, got 5")
    );
}

#[test]
fn porting_coverage_rejects_iqs9151_bit_porting_drift() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

ok_manifest = {
    "source_inventory": {
        "iqs9151_bit_flags": [
            "IQS9151_FLAG_A=128",
            "IQS9151_FLAG_B=15",
            "IQS9151_FLAG_C=512",
        ],
    },
    "iqs9151_bit_porting": [
        {"source_const": "IQS9151_FLAG_A", "source_value": 128, "status": "ported", "target_const": "FLAG_A"},
        {"source_const": "IQS9151_FLAG_B", "source_value": 15, "status": "ported", "target_const": "FLAG_B"},
        {"source_const": "IQS9151_FLAG_C", "source_value": 512, "status": "not_ported", "reason": "not needed"},
    ],
}
bad_manifest = {
    "source_inventory": ok_manifest["source_inventory"],
    "iqs9151_bit_porting": [
        {"source_const": "IQS9151_FLAG_A", "source_value": 128, "status": "ported", "target_const": "FLAG_A"},
        {"source_const": "IQS9151_FLAG_B", "source_value": 15, "status": "not_ported", "reason": ""},
        {"source_const": "IQS9151_FLAG_D", "source_value": 64, "status": "ported", "target_const": "FLAG_D"},
    ],
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    (root / "src").mkdir()
    (root / "src" / "iqs9151.rs").write_text('''
    pub const FLAG_A: u16 = 1 << 7;
    pub const FLAG_B: u16 = 0x000f;
    pub const FLAG_D: u16 = 1 << 5;
    ''')
    ok = pack(pc.check_iqs9151_bit_porting(ok_manifest, root))
    bad = pack(pc.check_iqs9151_bit_porting(bad_manifest, root))

print(json.dumps({"ok": ok, "bad": bad}))
"#,
    );

    assert!(
        output.status.success(),
        "IQS9151 bit porting parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_bit_flag_classification")
        .expect("missing ok bit classification result");
    assert_eq!(ok_inventory["kind"], "iqs9151_bit_porting");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);
    for result in parsed["ok"].as_array().unwrap() {
        assert_eq!(result["kind"], "iqs9151_bit_porting");
        assert_eq!(result["ok"], true);
    }

    let bad_inventory = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_upstream_bit_flag_classification")
        .expect("missing bad bit classification result");
    assert_eq!(bad_inventory["passed"].as_i64(), Some(2));
    assert_eq!(bad_inventory["total"].as_i64(), Some(3));
    assert_eq!(bad_inventory["ok"], false);
    assert!(
        bad_inventory["message"]
            .as_str()
            .unwrap()
            .contains("expected 'IQS9151_FLAG_C=512', got 'IQS9151_FLAG_D=64'")
    );

    let missing_reason = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_bit_porting.IQS9151_FLAG_B")
        .expect("missing no-reason bit result");
    assert_eq!(missing_reason["passed"].as_i64(), Some(1));
    assert_eq!(missing_reason["total"].as_i64(), Some(2));
    assert_eq!(missing_reason["ok"], false);
    assert!(
        missing_reason["message"]
            .as_str()
            .unwrap()
            .contains("has no reason")
    );

    let extra_source = parsed["bad"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "iqs9151_bit_porting.IQS9151_FLAG_D")
        .expect("missing extra-source bit result");
    assert_eq!(extra_source["passed"].as_i64(), Some(0));
    assert_eq!(extra_source["total"].as_i64(), Some(2));
    assert_eq!(extra_source["ok"], false);
    assert!(
        extra_source["message"]
            .as_str()
            .unwrap()
            .contains("source bit flag IQS9151_FLAG_D=64 is not in inventory")
    );
    assert!(
        extra_source["message"]
            .as_str()
            .unwrap()
            .contains("FLAG_D expected source value 64, got 32")
    );
}

#[test]
fn porting_coverage_rejects_rust_byte_array_drift() {
    let output = run_python(
        r#"
import hashlib
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

expected_hash = hashlib.sha256(bytes([0, 1, 255])).hexdigest()
manifest = {
    "rust_byte_arrays": [
        {
            "id": "ok_array",
            "target_file": "firmware.rs",
            "target_const": "IQS9151_CONFIG",
            "expected_len": 3,
            "expected_sha256": expected_hash,
        },
        {
            "id": "missing_array",
            "target_file": "firmware.rs",
            "target_const": "MISSING_ARRAY",
            "expected_len": 3,
            "expected_sha256": expected_hash,
        },
        {
            "id": "missing_file",
            "target_file": "missing.rs",
            "target_const": "IQS9151_CONFIG",
            "expected_len": 3,
            "expected_sha256": expected_hash,
        },
        {
            "id": "length_mismatch",
            "target_file": "bad_len.rs",
            "target_const": "IQS9151_CONFIG",
            "expected_len": 3,
            "expected_sha256": expected_hash,
        },
    ],
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    (root / "firmware.rs").write_text('''
    #[cfg(target_arch = "arm")]
    const IQS9151_CONFIG: [u8; 3] = [0x00, 1, 0xff];
    ''')
    (root / "bad_len.rs").write_text('const IQS9151_CONFIG: [u8; 4] = [0x00, 1, 0xff];')
    ok = pack(pc.check_rust_byte_arrays(manifest, root))

    (root / "firmware.rs").write_text('''
    const IQS9151_CONFIG: [u8; 3] = [0x00, 2, 0xff];
    ''')
    changed = pack(pc.check_rust_byte_arrays(manifest, root))

print(json.dumps({"ok": ok, "changed": changed}))
"#,
    );

    assert!(
        output.status.success(),
        "Rust byte array inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_array = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "ok_array")
        .expect("missing ok byte array result");
    assert_eq!(ok_array["kind"], "rust_byte_array");
    assert_eq!(ok_array["passed"].as_i64(), Some(2));
    assert_eq!(ok_array["total"].as_i64(), Some(2));
    assert_eq!(ok_array["ok"], true);

    let missing_array = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_array")
        .expect("missing missing-array result");
    assert_eq!(missing_array["kind"], "rust_byte_array");
    assert_eq!(missing_array["passed"].as_i64(), Some(0));
    assert_eq!(missing_array["total"].as_i64(), Some(2));
    assert_eq!(missing_array["ok"], false);
    assert!(
        missing_array["message"]
            .as_str()
            .unwrap()
            .contains("not found")
    );

    let missing_file = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "missing_file")
        .expect("missing missing-file result");
    assert_eq!(missing_file["kind"], "rust_byte_array");
    assert_eq!(missing_file["passed"].as_i64(), Some(0));
    assert_eq!(missing_file["total"].as_i64(), Some(2));
    assert_eq!(missing_file["ok"], false);
    assert!(
        missing_file["message"]
            .as_str()
            .unwrap()
            .contains("missing.rs")
    );

    let length_mismatch = parsed["ok"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "length_mismatch")
        .expect("missing length-mismatch result");
    assert_eq!(length_mismatch["kind"], "rust_byte_array");
    assert_eq!(length_mismatch["passed"].as_i64(), Some(0));
    assert_eq!(length_mismatch["total"].as_i64(), Some(2));
    assert_eq!(length_mismatch["ok"], false);
    assert!(
        length_mismatch["message"]
            .as_str()
            .unwrap()
            .contains("length mismatch")
    );

    let changed_array = parsed["changed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == "ok_array")
        .expect("missing changed byte array result");
    assert_eq!(changed_array["kind"], "rust_byte_array");
    assert_eq!(changed_array["passed"].as_i64(), Some(1));
    assert_eq!(changed_array["total"].as_i64(), Some(2));
    assert_eq!(changed_array["ok"], false);
    assert!(
        changed_array["message"]
            .as_str()
            .unwrap()
            .contains("sha256 expected")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_gpio_properties() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "gpio_properties": [{
            "source_file": "lalapadgen2.dtsi",
            "source_block": "kscan0",
            "source_property": "row-gpios",
            "expected": [
                "xiao_d:10:GPIO_ACTIVE_HIGH|GPIO_PULL_DOWN",
                "gpio1:1:GPIO_ACTIVE_HIGH|GPIO_PULL_DOWN",
            ],
        }],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.dtsi"
    fixture.write_text('''
    kscan0: kscan {
        row-gpios = <&xiao_d 10 (GPIO_ACTIVE_HIGH | GPIO_PULL_DOWN)>,
                    <&gpio1 1 (GPIO_ACTIVE_HIGH | GPIO_PULL_DOWN)>;
    };
    ''')
    ok = pack(pc.check_zmk_gpio_property_inventory(manifest, root))

    fixture.write_text('''
    kscan0: kscan {
        row-gpios = <&xiao_d 10 (GPIO_ACTIVE_HIGH | GPIO_PULL_DOWN)>,
                    <&gpio1 1 GPIO_ACTIVE_LOW>,
                    <&gpio0 2 GPIO_ACTIVE_HIGH>;
    };
    ''')
    changed = pack(pc.check_zmk_gpio_property_inventory(manifest, root))

    fixture.write_text('''
    kscan0: kscan {
        col-gpios = <&xiao_d 10 GPIO_ACTIVE_HIGH>;
    };
    ''')
    missing_property = pack(pc.check_zmk_gpio_property_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_gpio_property_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "missing_property": missing_property,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "GPIO property inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("gpio0:2:GPIO_ACTIVE_HIGH")
    );

    let missing_property_inventory = &parsed["missing_property"][0];
    assert_eq!(missing_property_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_property_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_property_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_property_inventory["ok"], false);
    assert!(
        missing_property_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid GPIO property source")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing GPIO property source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_west_manifest_items() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "west_manifest": [
            "remote:zmkfirmware:url-base=https://github.com/zmkfirmware",
            "project:zmk:remote=zmkfirmware:revision=v0.3.0:import=app/west.yml",
            "self:path=config",
        ],
    },
}

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "west.yml"
    fixture.write_text('''
manifest:
  remotes:
    - name: zmkfirmware
      url-base: https://github.com/zmkfirmware
  projects:
    - name: zmk
      remote: zmkfirmware
      revision: v0.3.0
      import: app/west.yml
  self:
    path: config
    ''')
    ok = pack(pc.check_west_manifest_inventory(manifest, root))

    fixture.write_text('''
manifest:
  remotes:
    - name: zmkfirmware
      url-base: https://github.com/zmkfirmware
  projects:
    - name: zmk
      remote: zmkfirmware
      revision: v0.3.1
      import: app/west.yml
    - name: zmk-new-module
      remote: zmkfirmware
      revision: main
  self:
    path: config
    ''')
    changed = pack(pc.check_west_manifest_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_west_manifest_inventory(manifest, root))

print(json.dumps({"ok": ok, "changed": changed, "missing_file": missing_file}))
"#,
    );

    assert!(
        output.status.success(),
        "west manifest inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(1));
    assert_eq!(changed_inventory["total"].as_i64(), Some(4));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("zmk-new-module")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing west manifest")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_build_files() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "build_files": [
            {
                "source_file": "../build.yaml",
                "expected": [
                    "include:board=seeeduino_xiao_ble:shield=lalapadgen2_right rgbled_adapter:snippet=studio-rpc-usb-uart",
                    "include:board=seeeduino_xiao_ble:shield=lalapadgen2_left rgbled_adapter",
                    "include:board=seeeduino_xiao_ble:shield=settings_reset",
                ],
            },
            {
                "source_file": "../zephyr/module.yml",
                "expected": ["build.settings.board_root=."],
            },
        ],
    },
}

def build(extra_include="", extra_top=""):
    return f'''
---
include:
  - board: seeeduino_xiao_ble
    shield: lalapadgen2_right rgbled_adapter
    snippet: studio-rpc-usb-uart
  - board: seeeduino_xiao_ble
    shield: lalapadgen2_left rgbled_adapter
  - board: seeeduino_xiao_ble
    shield: settings_reset
{extra_include}{extra_top}'''

def module(extra_settings="", extra_top=""):
    return f'''
---
build:
  settings:
    board_root: .
{extra_settings}{extra_top}'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    config = root / "config"
    config.mkdir()
    module_dir = root / "zephyr"
    module_dir.mkdir()
    build_fixture = root / "build.yaml"
    module_fixture = module_dir / "module.yml"
    build_fixture.write_text(build())
    module_fixture.write_text(module())
    ok = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.write_text(build('  - board: seeeduino_xiao_ble\n    shield: experimental_extra\n'))
    changed = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.write_text(build('    artifact-name: drift\n'))
    unknown_include_key = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.write_text(build(extra_top='other:\n  settings:\n    board_root: .\n'))
    wrong_section = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.write_text(build(extra_top='build:\n  settings:\n    board_root: .\n'))
    misplaced_board_root = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.write_text(build())
    module_fixture.write_text(module('    dts_root: .\n'))
    unknown_module_key = pack(pc.check_zmk_build_file_inventory(manifest, config))
    build_fixture.unlink()
    missing_file = pack(pc.check_zmk_build_file_inventory(manifest, config))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "unknown_include_key": unknown_include_key,
    "wrong_section": wrong_section,
    "misplaced_board_root": misplaced_board_root,
    "unknown_module_key": unknown_module_key,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "ZMK build file inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);
    let ok_module_inventory = &parsed["ok"][1];
    assert_eq!(ok_module_inventory["passed"].as_i64(), Some(1));
    assert_eq!(ok_module_inventory["total"].as_i64(), Some(1));
    assert_eq!(ok_module_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(3));
    assert_eq!(changed_inventory["total"].as_i64(), Some(4));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("experimental_extra")
    );

    let unknown_include_key_inventory = &parsed["unknown_include_key"][0];
    assert_eq!(unknown_include_key_inventory["passed"].as_i64(), Some(2));
    assert_eq!(unknown_include_key_inventory["total"].as_i64(), Some(4));
    assert_eq!(unknown_include_key_inventory["ok"], false);
    assert!(
        unknown_include_key_inventory["message"]
            .as_str()
            .unwrap()
            .contains("artifact-name")
    );

    let wrong_section_inventory = &parsed["wrong_section"][0];
    assert_eq!(wrong_section_inventory["passed"].as_i64(), Some(3));
    assert_eq!(wrong_section_inventory["total"].as_i64(), Some(6));
    assert_eq!(wrong_section_inventory["ok"], false);
    assert!(
        wrong_section_inventory["message"]
            .as_str()
            .unwrap()
            .contains("unknown.top_level.other")
    );

    let misplaced_board_root_inventory = &parsed["misplaced_board_root"][0];
    assert_eq!(misplaced_board_root_inventory["passed"].as_i64(), Some(3));
    assert_eq!(misplaced_board_root_inventory["total"].as_i64(), Some(4));
    assert_eq!(misplaced_board_root_inventory["ok"], false);
    assert!(
        misplaced_board_root_inventory["message"]
            .as_str()
            .unwrap()
            .contains("build.settings.board_root")
    );

    let unknown_module_key_inventory = &parsed["unknown_module_key"][1];
    assert_eq!(unknown_module_key_inventory["passed"].as_i64(), Some(1));
    assert_eq!(unknown_module_key_inventory["total"].as_i64(), Some(2));
    assert_eq!(unknown_module_key_inventory["ok"], false);
    assert!(
        unknown_module_key_inventory["message"]
            .as_str()
            .unwrap()
            .contains("build.settings.dts_root")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing ZMK build source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_workflow_files() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "workflow_files": [{
            "source_file": "../.github/workflows/build.yml",
            "expected": [
                "workflow.name=Build ZMK firmware",
                "workflow.on=push,pull_request,workflow_dispatch",
                "workflow.jobs.build.uses=zmkfirmware/zmk/.github/workflows/build-user-config.yml@v0.3",
            ],
        }],
    },
}

def workflow(uses="zmkfirmware/zmk/.github/workflows/build-user-config.yml@v0.3", extra_top=""):
    return f'''
name: Build ZMK firmware
on: [push, pull_request, workflow_dispatch]
{extra_top}
jobs:
  build:
    uses: {uses}
'''

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    config = root / "config"
    workflow_dir = root / ".github/workflows"
    config.mkdir()
    workflow_dir.mkdir(parents=True)
    fixture = workflow_dir / "build.yml"
    fixture.write_text(workflow())
    ok = pack(pc.check_zmk_workflow_file_inventory(manifest, config))
    fixture.write_text(workflow("zmkfirmware/zmk/.github/workflows/build-user-config.yml@main"))
    changed_ref = pack(pc.check_zmk_workflow_file_inventory(manifest, config))
    fixture.write_text(workflow(extra_top="permissions: read-all\n"))
    extra_top = pack(pc.check_zmk_workflow_file_inventory(manifest, config))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_workflow_file_inventory(manifest, config))

print(json.dumps({
    "ok": ok,
    "changed_ref": changed_ref,
    "extra_top": extra_top,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "ZMK workflow inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(3));
    assert_eq!(ok_inventory["total"].as_i64(), Some(3));
    assert_eq!(ok_inventory["ok"], true);

    let changed_ref_inventory = &parsed["changed_ref"][0];
    assert_eq!(changed_ref_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_ref_inventory["passed"].as_i64(), Some(2));
    assert_eq!(changed_ref_inventory["total"].as_i64(), Some(3));
    assert_eq!(changed_ref_inventory["ok"], false);
    assert!(
        changed_ref_inventory["message"]
            .as_str()
            .unwrap()
            .contains("@main")
    );

    let extra_top_inventory = &parsed["extra_top"][0];
    assert_eq!(extra_top_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_top_inventory["passed"].as_i64(), Some(2));
    assert_eq!(extra_top_inventory["total"].as_i64(), Some(4));
    assert_eq!(extra_top_inventory["ok"], false);
    assert!(
        extra_top_inventory["message"]
            .as_str()
            .unwrap()
            .contains("permissions")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(3));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing ZMK workflow source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_json_files() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "json_files": [{
            "source_file": "lalapadgen2.json",
            "expected": [
                "json.id=lalapadgen2",
                "json.name=lalapadgen2",
                "json.layouts=default_layout",
                "json.layouts.default_layout.name=default_layout",
                "json.layouts.default_layout.layout_count=2",
                "json.sensors=[]",
            ],
        }],
    },
}

def layout(extra_top=None, sensors=None):
    value = {
        "id": "lalapadgen2",
        "name": "lalapadgen2",
        "layouts": {
            "default_layout": {
                "name": "default_layout",
                "layout": [
                    {"row": 0, "col": 0, "x": 0, "y": 0},
                    {"row": 0, "col": 1, "x": 1, "y": 0},
                ],
            },
        },
        "sensors": [] if sensors is None else sensors,
    }
    if extra_top:
        value.update(extra_top)
    return json.dumps(value)

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.json"
    fixture.write_text(layout())
    ok = pack(pc.check_zmk_json_file_inventory(manifest, root))
    changed_name = json.loads(layout())
    changed_name["layouts"]["default_layout"]["name"] = "renamed_layout"
    fixture.write_text(json.dumps(changed_name))
    changed = pack(pc.check_zmk_json_file_inventory(manifest, root))
    fixture.write_text(layout(extra_top={"notes": "drift"}))
    extra_top = pack(pc.check_zmk_json_file_inventory(manifest, root))
    fixture.write_text(layout(sensors=[{"label": "encoder"}]))
    sensor_drift = pack(pc.check_zmk_json_file_inventory(manifest, root))
    fixture.write_text("{")
    invalid_json = pack(pc.check_zmk_json_file_inventory(manifest, root))
    fixture.write_text("[]")
    invalid_root = pack(pc.check_zmk_json_file_inventory(manifest, root))
    fixture.unlink()
    missing_file = pack(pc.check_zmk_json_file_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "changed": changed,
    "extra_top": extra_top,
    "sensor_drift": sensor_drift,
    "invalid_json": invalid_json,
    "invalid_root": invalid_root,
    "missing_file": missing_file,
}))
"#,
    );

    assert!(
        output.status.success(),
        "ZMK JSON inventory parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(6));
    assert_eq!(ok_inventory["total"].as_i64(), Some(6));
    assert_eq!(ok_inventory["ok"], true);

    let changed_inventory = &parsed["changed"][0];
    assert_eq!(changed_inventory["kind"], "zmk_inventory");
    assert_eq!(changed_inventory["passed"].as_i64(), Some(5));
    assert_eq!(changed_inventory["total"].as_i64(), Some(6));
    assert_eq!(changed_inventory["ok"], false);
    assert!(
        changed_inventory["message"]
            .as_str()
            .unwrap()
            .contains("renamed_layout")
    );

    let extra_top_inventory = &parsed["extra_top"][0];
    assert_eq!(extra_top_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_top_inventory["passed"].as_i64(), Some(6));
    assert_eq!(extra_top_inventory["total"].as_i64(), Some(7));
    assert_eq!(extra_top_inventory["ok"], false);
    assert!(
        extra_top_inventory["message"]
            .as_str()
            .unwrap()
            .contains("json.top_level.notes")
    );

    let sensor_drift_inventory = &parsed["sensor_drift"][0];
    assert_eq!(sensor_drift_inventory["kind"], "zmk_inventory");
    assert_eq!(sensor_drift_inventory["passed"].as_i64(), Some(5));
    assert_eq!(sensor_drift_inventory["total"].as_i64(), Some(6));
    assert_eq!(sensor_drift_inventory["ok"], false);
    assert!(
        sensor_drift_inventory["message"]
            .as_str()
            .unwrap()
            .contains("encoder")
    );

    let invalid_json_inventory = &parsed["invalid_json"][0];
    assert_eq!(invalid_json_inventory["kind"], "zmk_inventory");
    assert_eq!(invalid_json_inventory["passed"].as_i64(), Some(0));
    assert_eq!(invalid_json_inventory["total"].as_i64(), Some(6));
    assert_eq!(invalid_json_inventory["ok"], false);
    assert!(
        invalid_json_inventory["message"]
            .as_str()
            .unwrap()
            .contains("invalid ZMK JSON source file")
    );

    let invalid_root_inventory = &parsed["invalid_root"][0];
    assert_eq!(invalid_root_inventory["kind"], "zmk_inventory");
    assert_eq!(invalid_root_inventory["passed"].as_i64(), Some(0));
    assert_eq!(invalid_root_inventory["total"].as_i64(), Some(6));
    assert_eq!(invalid_root_inventory["ok"], false);
    assert!(
        invalid_root_inventory["message"]
            .as_str()
            .unwrap()
            .contains("must contain an object at the root")
    );

    let missing_file_inventory = &parsed["missing_file"][0];
    assert_eq!(missing_file_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_file_inventory["passed"].as_i64(), Some(0));
    assert_eq!(missing_file_inventory["total"].as_i64(), Some(6));
    assert_eq!(missing_file_inventory["ok"], false);
    assert!(
        missing_file_inventory["message"]
            .as_str()
            .unwrap()
            .contains("missing ZMK JSON source file")
    );
}

#[test]
fn porting_coverage_rejects_unclassified_zmk_json_layout_entries() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

manifest = {
    "source_inventory": {
        "json_layout_entries": [{
            "source_file": "lalapadgen2.json",
            "layout_name": "default_layout",
            "expected": [
                "default_layout[0]=row=0,col=0,x=0,y=1",
                "default_layout[1]=row=0,col=1,x=1,y=1",
            ],
        }],
    },
}

def layout(entries=None):
    return json.dumps({
        "layouts": {
            "default_layout": {
                "layout": [
                    {"row": 0, "col": 0, "x": 0, "y": 1},
                    {"row": 0, "col": 1, "x": 1, "y": 1},
                ] if entries is None else entries,
            },
        },
    })

def pack(results):
    return [result.__dict__ | {"ok": result.ok} for result in results]

with tempfile.TemporaryDirectory() as tempdir:
    root = Path(tempdir)
    fixture = root / "lalapadgen2.json"
    fixture.write_text(layout())
    ok = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))
    fixture.write_text(layout([
        {"row": 0, "col": 0, "x": 0, "y": 1},
        {"row": 0, "col": 1, "x": 1.5, "y": 1},
    ]))
    coordinate_drift = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))
    fixture.write_text(layout([
        {"row": 0, "col": 0, "x": 0, "y": 1},
        {"row": 0, "col": 1, "x": 1, "y": 1, "label": "extra"},
    ]))
    extra_attr = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))
    fixture.write_text(layout([
        {"row": 0, "col": 0, "x": 0, "y": 1},
    ]))
    missing_entry = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))
    fixture.write_text(layout([
        {"row": 0, "col": 0, "x": 0, "y": 1},
        "not-an-object",
    ]))
    malformed_entry = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))
    fixture.write_text(json.dumps({"layouts": {"default_layout": {"layout": {}}}}))
    invalid_layout = pack(pc.check_zmk_json_layout_entry_inventory(manifest, root))

print(json.dumps({
    "ok": ok,
    "coordinate_drift": coordinate_drift,
    "extra_attr": extra_attr,
    "missing_entry": missing_entry,
    "malformed_entry": malformed_entry,
    "invalid_layout": invalid_layout,
}))
"#,
    );

    assert!(
        output.status.success(),
        "ZMK JSON layout entry parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ok_inventory = &parsed["ok"][0];
    assert_eq!(ok_inventory["kind"], "zmk_inventory");
    assert_eq!(ok_inventory["passed"].as_i64(), Some(2));
    assert_eq!(ok_inventory["total"].as_i64(), Some(2));
    assert_eq!(ok_inventory["ok"], true);

    let coordinate_drift_inventory = &parsed["coordinate_drift"][0];
    assert_eq!(coordinate_drift_inventory["kind"], "zmk_inventory");
    assert_eq!(coordinate_drift_inventory["passed"].as_i64(), Some(1));
    assert_eq!(coordinate_drift_inventory["total"].as_i64(), Some(2));
    assert_eq!(coordinate_drift_inventory["ok"], false);
    assert!(
        coordinate_drift_inventory["message"]
            .as_str()
            .unwrap()
            .contains("x=1.5")
    );

    let extra_attr_inventory = &parsed["extra_attr"][0];
    assert_eq!(extra_attr_inventory["kind"], "zmk_inventory");
    assert_eq!(extra_attr_inventory["passed"].as_i64(), Some(1));
    assert_eq!(extra_attr_inventory["total"].as_i64(), Some(2));
    assert_eq!(extra_attr_inventory["ok"], false);
    assert!(
        extra_attr_inventory["message"]
            .as_str()
            .unwrap()
            .contains("label=\"extra\"")
    );

    let missing_entry_inventory = &parsed["missing_entry"][0];
    assert_eq!(missing_entry_inventory["kind"], "zmk_inventory");
    assert_eq!(missing_entry_inventory["passed"].as_i64(), Some(1));
    assert_eq!(missing_entry_inventory["total"].as_i64(), Some(2));
    assert_eq!(missing_entry_inventory["ok"], false);
    assert!(
        missing_entry_inventory["message"]
            .as_str()
            .unwrap()
            .contains("got None")
    );

    let malformed_entry_inventory = &parsed["malformed_entry"][0];
    assert_eq!(malformed_entry_inventory["kind"], "zmk_inventory");
    assert_eq!(malformed_entry_inventory["passed"].as_i64(), Some(1));
    assert_eq!(malformed_entry_inventory["total"].as_i64(), Some(2));
    assert_eq!(malformed_entry_inventory["ok"], false);
    assert!(
        malformed_entry_inventory["message"]
            .as_str()
            .unwrap()
            .contains("not-an-object")
    );

    let invalid_layout_inventory = &parsed["invalid_layout"][0];
    assert_eq!(invalid_layout_inventory["kind"], "zmk_inventory");
    assert_eq!(invalid_layout_inventory["passed"].as_i64(), Some(0));
    assert_eq!(invalid_layout_inventory["total"].as_i64(), Some(2));
    assert_eq!(invalid_layout_inventory["ok"], false);
    assert!(
        invalid_layout_inventory["message"]
            .as_str()
            .unwrap()
            .contains("default_layout:layout={}")
    );
}

#[test]
fn porting_coverage_parses_arbitrary_named_zmk_combos() {
    let output = run_python(
        r#"
import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("porting_coverage", "tools/porting_coverage.py")
pc = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = pc
spec.loader.exec_module(pc)

source = '''
/ {
    combos {
        compatible = "zmk,combos";
        plain-escape {
            bindings = <&kp ESCAPE>;
            key-positions = <0 1>;
        };
        left-escape {
            bindings = <&kp A>;
            key-positions = <2 3>;
        };
        right-escape {
            bindings = <&kp B>;
            key-positions = <4 5>;
        };
        labeled_tab: custom_node_name {
            bindings = <&kp TAB>;
            key-positions = <10 11>;
        };
        metadata {
            child {
                bindings = <&kp A>;
                key-positions = <2 3>;
            };
        };
    };
};
'''

combos = pc.zmk_combo_blocks(source)
print(json.dumps({
    "names": sorted(combos),
    "plain_escape": combos["plain-escape"],
    "left_escape": combos["left-escape"],
    "right_escape": combos["right-escape"],
    "labeled_tab": combos["labeled_tab"],
}))
"#,
    );

    assert!(
        output.status.success(),
        "combo parser check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        parsed["names"],
        serde_json::json!(["labeled_tab", "left-escape", "plain-escape", "right-escape"])
    );
    assert_eq!(parsed["plain_escape"][0], serde_json::json!([0, 1]));
    assert_eq!(parsed["plain_escape"][1], "Escape");
    assert_eq!(parsed["left_escape"][0], serde_json::json!([2, 3]));
    assert_eq!(parsed["left_escape"][1], "A");
    assert_eq!(parsed["right_escape"][0], serde_json::json!([4, 5]));
    assert_eq!(parsed["right_escape"][1], "B");
    assert_eq!(parsed["labeled_tab"][0], serde_json::json!([10, 11]));
    assert_eq!(parsed["labeled_tab"][1], "Tab");
}

fn keymap(value: &toml::Value) -> &Vec<toml::Value> {
    value["layout"]["keymap"].as_array().unwrap()
}

fn row_strings(layer: &toml::Value, row: usize) -> Vec<&str> {
    layer.as_array().unwrap()[row]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect()
}

#[test]
fn keyboard_layers_and_layer_keys_match_upstream_zmk_shape() {
    let parsed = keyboard_toml();
    let keymap = keymap(&parsed);

    assert_eq!(parsed["layout"]["layers"].as_integer(), Some(4));
    assert_eq!(
        row_strings(&keymap[0], 3),
        [
            "LCtrl",
            "LGui",
            "LAlt",
            "LCtrl",
            "LT(1, Space, FAST_LAYER)",
            "LShift",
            "Backspace",
            "LT(2, Enter, FAST_LAYER)",
            "MO(2)",
            "Language2",
            "Language1",
            "Backslash",
        ]
    );
    assert_eq!(
        row_strings(&keymap[1], 0),
        [
            "Kc1", "Kc2", "Kc3", "Kc4", "Kc5", "No", "No", "NumLock", "Kp7", "Kp8", "Kp9",
            "KpPlus",
        ]
    );
    assert_eq!(
        row_strings(&keymap[1], 3),
        [
            "_", "_", "_", "_", "_", "_", "Delete", "_", "_", "Kp0", "KpDot", "KpSlash",
        ]
    );
    assert_eq!(
        row_strings(&keymap[3], 2),
        [
            "F11", "F12", "F13", "F14", "F15", "No", "No", "User9", "User10", "User11", "User12",
            "User13",
        ]
    );
    assert_eq!(
        row_strings(&keymap[3], 1),
        [
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "No",
            "No",
            "Reboot",
            "Bootloader",
            "User6",
            "User8",
            "_",
        ]
    );

    assert_eq!(
        parsed["behavior"]["tri_layer"]["lower"].as_integer(),
        Some(1)
    );
    assert_eq!(
        parsed["behavior"]["tri_layer"]["upper"].as_integer(),
        Some(2)
    );
    assert_eq!(
        parsed["behavior"]["tri_layer"]["adjust"].as_integer(),
        Some(3)
    );
    assert_eq!(
        parsed["behavior"]["morse"]["enable_flow_tap"].as_bool(),
        Some(false)
    );
    assert_eq!(
        parsed["behavior"]["morse"]["prior_idle_time"].as_str(),
        Some("125ms")
    );
    assert_eq!(
        parsed["behavior"]["morse"]["normal_mode"].as_bool(),
        Some(true)
    );
    assert_eq!(
        parsed["behavior"]["morse"]["profiles"]["FAST_LAYER"]["hold_on_other_press"].as_bool(),
        Some(true)
    );
    assert_eq!(
        parsed["behavior"]["morse"]["hold_timeout"].as_str(),
        Some("200ms")
    );
    assert_eq!(
        parsed["behavior"]["morse"]["gap_timeout"].as_str(),
        Some("200ms")
    );
}

#[test]
fn upstream_zmk_combo_set_stays_minimal() {
    let parsed = keyboard_toml();
    let combos = parsed["behavior"]["combo"]["combos"].as_array().unwrap();
    let actual: Vec<(Vec<&str>, &str, i64)> = combos
        .iter()
        .map(|combo| {
            let actions = combo["actions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|action| action.as_str().unwrap())
                .collect();
            (
                actions,
                combo["output"].as_str().unwrap(),
                combo["layer"].as_integer().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        actual,
        [
            (vec!["Q", "W"], "Escape", 0),
            (vec!["A", "S"], "Tab", 0),
            (vec!["J", "K"], "Language1", 0),
            (vec!["D", "F"], "Language2", 0),
        ]
    );
}

#[test]
fn trackpad_virtual_positions_match_zmk_input_btn_order() {
    let parsed = keyboard_toml();
    let keymap = keymap(&parsed);
    let vial = vial_json();
    let vial_layout = vial["layouts"]["keymap"].as_array().unwrap();
    let expected = [
        (
            TrackpadSide::Left,
            TrackpadButton::LeftClick,
            5,
            0,
            "MouseBtn1",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::RightClick,
            5,
            1,
            "MouseBtn2",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::MiddleClick,
            5,
            2,
            "MouseBtn3",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::GestureLeft,
            6,
            0,
            "MouseBtn4",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::GestureRight,
            6,
            1,
            "MouseBtn5",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::GestureUp,
            6,
            2,
            "WM(Tab, LGui)",
        ),
        (
            TrackpadSide::Left,
            TrackpadButton::GestureDown,
            6,
            3,
            "WM(D, LGui)",
        ),
        (TrackpadSide::Left, TrackpadButton::Pinch, 6, 4, "LCtrl"),
        (
            TrackpadSide::Right,
            TrackpadButton::LeftClick,
            5,
            9,
            "MouseBtn1",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::RightClick,
            5,
            10,
            "MouseBtn2",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::MiddleClick,
            5,
            11,
            "MouseBtn3",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::GestureLeft,
            6,
            7,
            "MouseBtn4",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::GestureRight,
            6,
            8,
            "MouseBtn5",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::GestureUp,
            6,
            9,
            "WM(Tab, LGui)",
        ),
        (
            TrackpadSide::Right,
            TrackpadButton::GestureDown,
            6,
            10,
            "WM(D, LGui)",
        ),
        (TrackpadSide::Right, TrackpadButton::Pinch, 6, 11, "LCtrl"),
    ];

    for (side, button, row, col, key) in expected {
        let position = VirtualKeyPosition { row, col };
        assert_eq!(trackpad_button_position(side, button), position);
        assert_eq!(keymap[0][row as usize][col as usize].as_str(), Some(key));
        let position_text = format!("{row},{col}");
        assert!(vial_layout.iter().any(|row| {
            row.as_array().unwrap().iter().any(|item| {
                item.as_str()
                    .is_some_and(|value| value == position_text.as_str())
            })
        }));
    }
}
