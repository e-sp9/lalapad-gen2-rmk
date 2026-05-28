use crate::iqs9151::{TrackpadButton, TrackpadSide, VirtualKeyPosition, trackpad_button_position};
use std::process::Command;

const KEYBOARD_TOML: &str = include_str!("../keyboard.toml");
const PORTING_COVERAGE_MANIFEST_TOML: &str =
    include_str!("../tools/porting_coverage_manifest.toml");
const VIAL_JSON: &str = include_str!("../vial.json");

fn keyboard_toml() -> toml::Value {
    toml::from_str(KEYBOARD_TOML).unwrap()
}

fn porting_coverage_manifest_toml() -> toml::Value {
    toml::from_str(PORTING_COVERAGE_MANIFEST_TOML).unwrap()
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

fn run_python(script: &str) -> std::process::Output {
    Command::new("python3")
        .arg("-c")
        .arg(script)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn default_zmk_config_dir() -> Option<std::path::PathBuf> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../zmk-config-LalaPadGen2/config");
    path.join("lalapadgen2.keymap").exists().then_some(path)
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
    let keyboard = keyboard_toml();
    let layers = keyboard["layout"]["layers"].as_integer().unwrap();
    let rows = keyboard["layout"]["rows"].as_integer().unwrap();
    let expected_keymap_shape_total = 1 + layers + layers * rows;
    let expected_combo_total = keyboard["behavior"]["combo"]["combos"]
        .as_array()
        .unwrap()
        .len() as i64;

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

    if default_zmk_config_dir().is_some() {
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
    }
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
