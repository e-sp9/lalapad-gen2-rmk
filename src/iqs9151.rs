//! IQS9151 porting helpers.
//!
//! The upstream ZMK driver emits `INPUT_BTN_0..7` for trackpad clicks and
//! gestures, then maps those events to virtual key positions. This module keeps
//! the RMK-side mapping and frame layout explicit while the runtime I2C driver
//! is being ported.

pub const I2C_ADDRESS: u8 = 0x56;
pub const PRODUCT_NUMBER: u16 = 0x09bc;

pub const ADDR_PRODUCT_NUMBER: u16 = 0x1000;
pub const ADDR_RELATIVE_X: u16 = 0x1014;
pub const ADDR_RELATIVE_Y: u16 = 0x1016;
pub const ADDR_SINGLE_GESTURES: u16 = 0x101c;
pub const ADDR_TWO_FINGER_GESTURES: u16 = 0x101e;
pub const ADDR_INFO_FLAGS: u16 = 0x1020;
pub const ADDR_TRACKPAD_FLAGS: u16 = 0x1022;
pub const ADDR_FINGER1_X: u16 = 0x1024;
pub const ADDR_FINGER1_Y: u16 = 0x1026;
pub const ADDR_FINGER2_X: u16 = 0x102c;
pub const ADDR_FINGER2_Y: u16 = 0x102e;
pub const ADDR_TOUCH_STATUS: u16 = 0x105c;

pub const COORD_BLOCK_START: u16 = ADDR_RELATIVE_X;
pub const COORD_BLOCK_LENGTH: usize = 0x48;

pub const INFO_SHOW_RESET: u16 = 1 << 7;
pub const INFO_GLOBAL_TP_TOUCH: u16 = 1 << 9;
pub const INFO_TP_TOUCH_TOGGLED: u16 = 1 << 13;

pub const TP_MOVEMENT_DETECTED: u16 = 1 << 4;
pub const TP_FINGER_COUNT_MASK: u16 = 0x000f;
pub const TP_FINGER1_CONFIDENCE: u16 = 1 << 8;
pub const TP_FINGER2_CONFIDENCE: u16 = 1 << 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadButton {
    LeftClick,
    RightClick,
    MiddleClick,
    GestureLeft,
    GestureRight,
    GestureUp,
    GestureDown,
    Pinch,
}

impl TrackpadButton {
    pub const fn from_input_btn_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::LeftClick),
            1 => Some(Self::RightClick),
            2 => Some(Self::MiddleClick),
            3 => Some(Self::GestureLeft),
            4 => Some(Self::GestureRight),
            5 => Some(Self::GestureUp),
            6 => Some(Self::GestureDown),
            7 => Some(Self::Pinch),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualKeyPosition {
    pub row: u8,
    pub col: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadButtonEvent {
    pub button: TrackpadButton,
    pub position: VirtualKeyPosition,
    pub pressed: bool,
}

pub const fn trackpad_button_position(
    side: TrackpadSide,
    button: TrackpadButton,
) -> VirtualKeyPosition {
    match (side, button) {
        (TrackpadSide::Left, TrackpadButton::LeftClick) => VirtualKeyPosition { row: 5, col: 0 },
        (TrackpadSide::Left, TrackpadButton::RightClick) => VirtualKeyPosition { row: 5, col: 1 },
        (TrackpadSide::Left, TrackpadButton::MiddleClick) => VirtualKeyPosition { row: 5, col: 2 },
        (TrackpadSide::Left, TrackpadButton::GestureLeft) => VirtualKeyPosition { row: 6, col: 0 },
        (TrackpadSide::Left, TrackpadButton::GestureRight) => VirtualKeyPosition { row: 6, col: 1 },
        (TrackpadSide::Left, TrackpadButton::GestureUp) => VirtualKeyPosition { row: 6, col: 2 },
        (TrackpadSide::Left, TrackpadButton::GestureDown) => VirtualKeyPosition { row: 6, col: 3 },
        (TrackpadSide::Left, TrackpadButton::Pinch) => VirtualKeyPosition { row: 6, col: 4 },

        (TrackpadSide::Right, TrackpadButton::LeftClick) => VirtualKeyPosition { row: 5, col: 9 },
        (TrackpadSide::Right, TrackpadButton::RightClick) => VirtualKeyPosition { row: 5, col: 10 },
        (TrackpadSide::Right, TrackpadButton::MiddleClick) => {
            VirtualKeyPosition { row: 5, col: 11 }
        }
        (TrackpadSide::Right, TrackpadButton::GestureLeft) => VirtualKeyPosition { row: 6, col: 7 },
        (TrackpadSide::Right, TrackpadButton::GestureRight) => {
            VirtualKeyPosition { row: 6, col: 8 }
        }
        (TrackpadSide::Right, TrackpadButton::GestureUp) => VirtualKeyPosition { row: 6, col: 9 },
        (TrackpadSide::Right, TrackpadButton::GestureDown) => {
            VirtualKeyPosition { row: 6, col: 10 }
        }
        (TrackpadSide::Right, TrackpadButton::Pinch) => VirtualKeyPosition { row: 6, col: 11 },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadButtonState {
    side: TrackpadSide,
    pressed_bits: u8,
}

impl TrackpadButtonState {
    pub const fn new(side: TrackpadSide) -> Self {
        Self {
            side,
            pressed_bits: 0,
        }
    }

    pub const fn pressed_bits(self) -> u8 {
        self.pressed_bits
    }

    pub fn update(&mut self, next_pressed_bits: u8) -> TrackpadButtonEvents {
        let previous_bits = self.pressed_bits;
        self.pressed_bits = next_pressed_bits & 0xff;

        TrackpadButtonEvents {
            side: self.side,
            changed_bits: previous_bits ^ self.pressed_bits,
            pressed_bits: self.pressed_bits,
            next_code: 0,
        }
    }
}

pub struct TrackpadButtonEvents {
    side: TrackpadSide,
    changed_bits: u8,
    pressed_bits: u8,
    next_code: u8,
}

impl Iterator for TrackpadButtonEvents {
    type Item = TrackpadButtonEvent;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_code < 8 {
            let code = self.next_code;
            self.next_code += 1;
            let bit = 1 << code;

            if self.changed_bits & bit == 0 {
                continue;
            }

            let button = TrackpadButton::from_input_btn_code(code)?;
            return Some(TrackpadButtonEvent {
                button,
                position: trackpad_button_position(self.side, button),
                pressed: self.pressed_bits & bit != 0,
            });
        }

        None
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FingerPosition {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoordinateFrame {
    pub relative_x: i16,
    pub relative_y: i16,
    pub single_gestures: u16,
    pub two_finger_gestures: u16,
    pub info_flags: u16,
    pub trackpad_flags: u16,
    pub finger1: FingerPosition,
    pub finger2: FingerPosition,
}

impl CoordinateFrame {
    pub fn parse(block: &[u8; COORD_BLOCK_LENGTH]) -> Self {
        Self {
            relative_x: read_i16_le(block, 0x00),
            relative_y: read_i16_le(block, 0x02),
            single_gestures: read_u16_le(block, 0x08),
            two_finger_gestures: read_u16_le(block, 0x0a),
            info_flags: read_u16_le(block, 0x0c),
            trackpad_flags: read_u16_le(block, 0x0e),
            finger1: FingerPosition {
                x: read_u16_le(block, 0x10),
                y: read_u16_le(block, 0x12),
            },
            finger2: FingerPosition {
                x: read_u16_le(block, 0x18),
                y: read_u16_le(block, 0x1a),
            },
        }
    }

    pub const fn finger_count(self) -> u8 {
        (self.trackpad_flags & TP_FINGER_COUNT_MASK) as u8
    }

    pub const fn show_reset(self) -> bool {
        self.info_flags & INFO_SHOW_RESET != 0
    }
}

const fn read_u16_le(bytes: &[u8; COORD_BLOCK_LENGTH], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

const fn read_i16_le(bytes: &[u8; COORD_BLOCK_LENGTH], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_input_buttons_to_left_virtual_positions() {
        let expected = [
            (TrackpadButton::LeftClick, (5, 0)),
            (TrackpadButton::RightClick, (5, 1)),
            (TrackpadButton::MiddleClick, (5, 2)),
            (TrackpadButton::GestureLeft, (6, 0)),
            (TrackpadButton::GestureRight, (6, 1)),
            (TrackpadButton::GestureUp, (6, 2)),
            (TrackpadButton::GestureDown, (6, 3)),
            (TrackpadButton::Pinch, (6, 4)),
        ];

        for (button, (row, col)) in expected {
            assert_eq!(
                trackpad_button_position(TrackpadSide::Left, button),
                VirtualKeyPosition { row, col }
            );
        }
    }

    #[test]
    fn maps_input_buttons_to_right_virtual_positions() {
        let expected = [
            (TrackpadButton::LeftClick, (5, 9)),
            (TrackpadButton::RightClick, (5, 10)),
            (TrackpadButton::MiddleClick, (5, 11)),
            (TrackpadButton::GestureLeft, (6, 7)),
            (TrackpadButton::GestureRight, (6, 8)),
            (TrackpadButton::GestureUp, (6, 9)),
            (TrackpadButton::GestureDown, (6, 10)),
            (TrackpadButton::Pinch, (6, 11)),
        ];

        for (button, (row, col)) in expected {
            assert_eq!(
                trackpad_button_position(TrackpadSide::Right, button),
                VirtualKeyPosition { row, col }
            );
        }
    }

    #[test]
    fn parses_coordinate_block_offsets() {
        let mut block = [0; COORD_BLOCK_LENGTH];
        block[0x00..0x02].copy_from_slice(&(-12i16).to_le_bytes());
        block[0x02..0x04].copy_from_slice(&(34i16).to_le_bytes());
        block[0x08..0x0a].copy_from_slice(&0x0001u16.to_le_bytes());
        block[0x0a..0x0c].copy_from_slice(&0x0040u16.to_le_bytes());
        block[0x0c..0x0e].copy_from_slice(&INFO_SHOW_RESET.to_le_bytes());
        block[0x0e..0x10].copy_from_slice(&(TP_FINGER1_CONFIDENCE | 2).to_le_bytes());
        block[0x10..0x12].copy_from_slice(&123u16.to_le_bytes());
        block[0x12..0x14].copy_from_slice(&456u16.to_le_bytes());
        block[0x18..0x1a].copy_from_slice(&789u16.to_le_bytes());
        block[0x1a..0x1c].copy_from_slice(&1024u16.to_le_bytes());

        let frame = CoordinateFrame::parse(&block);

        assert_eq!(frame.relative_x, -12);
        assert_eq!(frame.relative_y, 34);
        assert_eq!(frame.single_gestures, 0x0001);
        assert_eq!(frame.two_finger_gestures, 0x0040);
        assert!(frame.show_reset());
        assert_eq!(frame.finger_count(), 2);
        assert_eq!(frame.finger1, FingerPosition { x: 123, y: 456 });
        assert_eq!(frame.finger2, FingerPosition { x: 789, y: 1024 });
    }

    #[test]
    fn tracks_button_press_and_release_edges() {
        let mut state = TrackpadButtonState::new(TrackpadSide::Right);

        let pressed: std::vec::Vec<_> = state.update(0b0000_0011).collect();
        assert_eq!(
            pressed,
            std::vec![
                TrackpadButtonEvent {
                    button: TrackpadButton::LeftClick,
                    position: VirtualKeyPosition { row: 5, col: 9 },
                    pressed: true,
                },
                TrackpadButtonEvent {
                    button: TrackpadButton::RightClick,
                    position: VirtualKeyPosition { row: 5, col: 10 },
                    pressed: true,
                },
            ]
        );

        let changed: std::vec::Vec<_> = state.update(0b1000_0010).collect();
        assert_eq!(
            changed,
            std::vec![
                TrackpadButtonEvent {
                    button: TrackpadButton::LeftClick,
                    position: VirtualKeyPosition { row: 5, col: 9 },
                    pressed: false,
                },
                TrackpadButtonEvent {
                    button: TrackpadButton::Pinch,
                    position: VirtualKeyPosition { row: 6, col: 11 },
                    pressed: true,
                },
            ]
        );
    }
}
