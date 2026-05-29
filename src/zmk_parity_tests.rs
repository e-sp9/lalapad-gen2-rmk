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
    let manifest = porting_coverage_manifest_toml();
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
    (root / "source.conf").write_text("CONFIG_INPUT_IQS9151_RESOLUTION_X=2457\n")
    firmware = root / "firmware.rs"
    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
    const DEFAULT_CURSOR_INERTIA_ENABLED: bool = true;
    pub const INFO_SHOW_RESET: u16 = 1 << 7;
    pub const DEFAULT_X_RESOLUTION: u16 = 2457;
    ''')
    ok = pack(pc.check_rust_const_values(manifest, root, root))

    firmware.write_text('''
    pub const ADDR_PRODUCT_NUMBER: u16 = 0x1002;
    const DEFAULT_CURSOR_INERTIA_ENABLED: bool = false;
    pub const INFO_SHOW_RESET: u16 = 1 << 6;
    pub const DEFAULT_X_RESOLUTION: u16 = 2000;
    ''')
    (root / "source.conf").write_text("CONFIG_INPUT_IQS9151_RESOLUTION_X=2000\n")
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
