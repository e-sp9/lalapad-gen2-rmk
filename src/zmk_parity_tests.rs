use crate::iqs9151::{TrackpadButton, TrackpadSide, VirtualKeyPosition, trackpad_button_position};

const KEYBOARD_TOML: &str = include_str!("../keyboard.toml");
const VIAL_JSON: &str = include_str!("../vial.json");

fn keyboard_toml() -> toml::Value {
    toml::from_str(KEYBOARD_TOML).unwrap()
}

fn vial_json() -> serde_json::Value {
    serde_json::from_str(VIAL_JSON).unwrap()
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
            "MO(1)",
            "Space",
            "LShift",
            "Backspace",
            "Enter",
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
        Some(true)
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
