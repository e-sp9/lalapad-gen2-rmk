pub mod common;

use rmk::config::{BehaviorConfig, MorsesConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::KeyCode;
use rmk::{a, k, layer};
use rmk_types::action::{MorseMode, MorseProfile};
use rusty_fork::rusty_fork_test;

use crate::common::wrap_keymap;

const SHIPPED_KEYBOARD_TOML: &str = include_str!("../../../keyboard.toml");

const FAST_LAYER: MorseProfile = MorseProfile::new(
    Some(false),
    Some(MorseMode::HoldOnOtherPress),
    Some(200),
    Some(200),
);

const fn lt(layer: u8, key: KeyCode) -> KeyAction {
    KeyAction::TapHold(Action::Key(key), Action::LayerOn(layer), FAST_LAYER)
}

#[rustfmt::skip]
fn lalapad_keymap() -> [[[KeyAction; 12]; 7]; 4] {
    [
        layer!([
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Y), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(H), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(N), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), lt(1, KeyCode::Space), a!(No), a!(No), lt(2, KeyCode::Enter), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)]
        ]),
        layer!([
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(NumLock), k!(Kp7), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(Transparent), k!(Kp4), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), k!(Kp0), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)]
        ]),
        layer!([
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(PageUp), k!(Home), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(PageDown), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)]
        ]),
        layer!([
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(User7), k!(User0), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Reboot), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)]
        ]),
    ]
}

fn create_lalapad_keyboard() -> Keyboard<'static, 7, 12, 4> {
    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> =
        static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(BehaviorConfig {
        tri_layer: Some([1, 2, 3]),
        morse: MorsesConfig {
            enable_flow_tap: false,
            prior_idle_time: embassy_time::Duration::from_millis(125),
            default_profile: FAST_LAYER,
            ..Default::default()
        },
        ..Default::default()
    });
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<7, 12>> =
        static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(
        lalapad_keymap(),
        per_key_config,
        behavior_config,
    ))
}

rusty_fork_test! {
    #[test]
    fn shipped_keyboard_toml_keeps_thumb_layer_tap_bindings() {
        assert!(
            SHIPPED_KEYBOARD_TOML.contains("\"LT(1, Space, FAST_LAYER)\""),
            "keyboard.toml must keep Space as the layer-1 FAST_LAYER tap-hold"
        );
        assert!(
            SHIPPED_KEYBOARD_TOML.contains("\"LT(2, Enter, FAST_LAYER)\""),
            "keyboard.toml must keep Enter as the layer-2 FAST_LAYER tap-hold"
        );
    }

    #[test]
    fn space_tap_is_space() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [3, 4, false, 100],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Space), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn enter_tap_is_enter() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 7, true, 10],
                [3, 7, false, 100],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Enter), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn space_hold_y_selects_secondary_layer() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [0, 7, true, 10],
                [0, 7, false, 100],
                [3, 4, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(NumLock), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn space_hold_u_selects_keypad_7() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [0, 8, true, 10],
                [0, 8, false, 100],
                [3, 4, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp7), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn space_hold_j_selects_keypad_4() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [1, 8, true, 10],
                [1, 8, false, 100],
                [3, 4, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp4), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn space_hold_h_falls_back_to_base() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [1, 7, true, 10],
                [1, 7, false, 100],
                [3, 4, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(H), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn enter_hold_y_selects_tertiary_layer() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 7, true, 10],
                [0, 7, true, 10],
                [0, 7, false, 100],
                [3, 7, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(PageUp), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn enter_hold_u_selects_home() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 7, true, 10],
                [0, 8, true, 10],
                [0, 8, false, 100],
                [3, 7, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Home), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn enter_hold_h_selects_page_down() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 7, true, 10],
                [1, 7, true, 10],
                [1, 7, false, 100],
                [3, 7, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(PageDown), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn enter_hold_n_falls_back_to_base() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 7, true, 10],
                [2, 7, true, 10],
                [2, 7, false, 100],
                [3, 7, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(N), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn space_enter_hold_keypad_zero_falls_through_system_transparency() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [3, 4, true, 10],
                [3, 7, true, 10],
                [3, 9, true, 10],
                [3, 9, false, 100],
                [3, 7, false, 10],
                [3, 4, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Kp0), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }
}
