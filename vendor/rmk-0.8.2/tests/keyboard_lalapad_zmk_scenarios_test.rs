pub mod common;

use embassy_time::{Duration, Timer};
use rmk::channel::{KEY_EVENT_CHANNEL, KEYBOARD_REPORT_CHANNEL};
use rmk::combo::{Combo, ComboConfig};
use rmk::config::{BehaviorConfig, CombosConfig, MorsesConfig, PositionalConfig};
use rmk::event::KeyboardEvent;
use rmk::hid::Report;
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::KeyCode;
use rmk::{a, k, layer};
use rmk_types::action::{MorseMode, MorseProfile};
use rusty_fork::rusty_fork_test;

use crate::common::{TestKeyPress, wrap_keymap};

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
            [k!(Q), k!(W), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Y), a!(No), a!(No), a!(No), a!(No)],
            [k!(A), k!(S), k!(D), k!(F), a!(No), a!(No), a!(No), k!(H), k!(J), k!(K), a!(No), a!(No)],
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
        combo: lalapad_combos_config(),
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

fn lalapad_combos_config() -> CombosConfig {
    CombosConfig {
        combos: [
            Some(Combo::new(ComboConfig::new(
                [k!(Q), k!(W)].to_vec(),
                k!(Escape),
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [k!(A), k!(S)].to_vec(),
                k!(Tab),
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [k!(J), k!(K)].to_vec(),
                k!(Language1),
                Some(0),
            ))),
            Some(Combo::new(ComboConfig::new(
                [k!(D), k!(F)].to_vec(),
                k!(Language2),
                Some(0),
            ))),
            None,
            None,
            None,
            None,
        ],
        timeout: Duration::from_millis(50),
    }
}

fn assert_no_pending_hid_reports() {
    while let Ok(report) = KEYBOARD_REPORT_CHANNEL.try_receive() {
        match report {
            Report::KeyboardReport(report) => {
                panic!("unexpected keyboard report: {:?}", report)
            }
            report => panic!("unexpected HID report: {:?}", report),
        }
    }
}

async fn run_key_sequence_expect_no_keyboard_reports<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    key_sequence: &[TestKeyPress],
) {
    KEY_EVENT_CHANNEL.clear();
    KEYBOARD_REPORT_CHANNEL.clear();
    static REPORT_GRACE: Duration = Duration::from_millis(250);

    for key in key_sequence {
        Timer::after(Duration::from_millis(key.delay)).await;
        keyboard
            .process_inner(KeyboardEvent::key(key.row, key.col, key.pressed))
            .await;
        assert_no_pending_hid_reports();
    }
    Timer::after(REPORT_GRACE).await;
    assert_no_pending_hid_reports();
    if !keyboard.held_buffer.is_empty() {
        panic!(
            "leak after buffer cleanup, buffer contains {:?}",
            keyboard.held_buffer
        );
    }
}

macro_rules! key_sequence_no_keyboard_reports_test {
    (
        expected_action: [$layer:expr, $row:expr, $col:expr] => $expected_action:expr,
        sequence: [$([$seq_row:expr, $seq_col:expr, $pressed:expr, $delay:expr]),* $(,)?]
    ) => {
        ::embassy_futures::block_on(async {
            let mut keyboard = create_lalapad_keyboard();
            assert_eq!(lalapad_keymap()[$layer][$row][$col], $expected_action);
            let sequence = vec![
                $(
                    TestKeyPress {
                        row: $seq_row,
                        col: $seq_col,
                        pressed: $pressed,
                        delay: $delay,
                    },
                )*
            ];

            run_key_sequence_expect_no_keyboard_reports(&mut keyboard, &sequence).await;
        });
    };
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
    fn runtime_keymap_keeps_system_tri_layer_actions() {
        let keymap = lalapad_keymap();
        assert_eq!(keymap[3][0][7], k!(User7));
        assert_eq!(keymap[3][0][8], k!(User0));
        assert_eq!(keymap[3][1][7], k!(Reboot));
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
    fn combo_q_w_outputs_escape() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [0, 0, true, 10],
                [0, 1, true, 10],
                [0, 0, false, 10],
                [0, 1, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Escape), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn combo_a_s_outputs_tab() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [1, 0, true, 10],
                [1, 1, true, 10],
                [1, 0, false, 10],
                [1, 1, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Tab), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn combo_j_k_outputs_language1() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [1, 8, true, 10],
                [1, 9, true, 10],
                [1, 8, false, 10],
                [1, 9, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Language1), 0, 0, 0, 0, 0]],
                [0, [0, 0, 0, 0, 0, 0]],
            ]
        };
    }

    #[test]
    fn combo_d_f_outputs_language2() {
        key_sequence_test! {
            keyboard: create_lalapad_keyboard(),
            sequence: [
                [1, 2, true, 10],
                [1, 3, true, 10],
                [1, 2, false, 10],
                [1, 3, false, 10],
            ],
            expected_reports: [
                [0, [kc_to_u8!(Language2), 0, 0, 0, 0, 0]],
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
    fn space_enter_hold_y_selects_system_layer_user7_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 0, 7] => k!(User7),
            sequence: [
                [3, 4, true, 10],
                [3, 7, true, 10],
                [0, 7, true, 10],
                [0, 7, false, 100],
                [3, 7, false, 10],
                [3, 4, false, 10],
            ]
        };
    }

    #[test]
    fn space_enter_hold_u_selects_bt_profile_0_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 0, 8] => k!(User0),
            sequence: [
                [3, 4, true, 10],
                [3, 7, true, 10],
                [0, 8, true, 10],
                [0, 8, false, 100],
                [3, 7, false, 10],
                [3, 4, false, 10],
            ]
        };
    }

    #[test]
    fn space_enter_hold_h_selects_reboot_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 1, 7] => k!(Reboot),
            sequence: [
                [3, 4, true, 10],
                [3, 7, true, 10],
                [1, 7, true, 10],
                [1, 7, false, 100],
                [3, 7, false, 10],
                [3, 4, false, 10],
            ]
        };
    }

    #[test]
    fn enter_space_hold_y_selects_system_layer_user7_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 0, 7] => k!(User7),
            sequence: [
                [3, 7, true, 10],
                [3, 4, true, 10],
                [0, 7, true, 10],
                [0, 7, false, 100],
                [3, 4, false, 10],
                [3, 7, false, 10],
            ]
        };
    }

    #[test]
    fn enter_space_hold_u_selects_bt_profile_0_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 0, 8] => k!(User0),
            sequence: [
                [3, 7, true, 10],
                [3, 4, true, 10],
                [0, 8, true, 10],
                [0, 8, false, 100],
                [3, 4, false, 10],
                [3, 7, false, 10],
            ]
        };
    }

    #[test]
    fn enter_space_hold_h_selects_reboot_without_keyboard_report() {
        key_sequence_no_keyboard_reports_test! {
            expected_action: [3, 1, 7] => k!(Reboot),
            sequence: [
                [3, 7, true, 10],
                [3, 4, true, 10],
                [1, 7, true, 10],
                [1, 7, false, 100],
                [3, 4, false, 10],
                [3, 7, false, 10],
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
