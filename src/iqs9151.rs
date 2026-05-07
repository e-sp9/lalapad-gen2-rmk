//! IQS9151 porting helpers.
//!
//! The upstream ZMK driver emits `INPUT_BTN_0..7` for trackpad clicks and
//! gestures, then maps those events to virtual key positions. This module keeps
//! the RMK-side mapping and frame layout explicit while the runtime I2C driver
//! is being ported.

use core::{
    future::Future,
    sync::atomic::{AtomicI32, AtomicU8, AtomicU16, Ordering},
};

use embassy_time::{Duration, Instant, Timer, with_timeout};
use embedded_hal_async::{digital::Wait, i2c::I2c};
use rmk::{
    channel::{EVENT_CHANNEL, KEY_EVENT_CHANNEL, KEYBOARD_REPORT_CHANNEL},
    controller::Controller,
    event::{Event, KeyboardEvent},
    hid::Report,
    input_device::InputDevice,
};
use usbd_hid::descriptor::MouseReport;

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
pub const ADDR_ALP_COMPENSATION: u16 = 0x115c;
pub const ADDR_SETTINGS_MINOR: u16 = 0x1178;
pub const ADDR_TIMING_SETTINGS: u16 = 0x11a2;
pub const ADDR_TRACKPAD_ATI_TARGET: u16 = 0x1196;
pub const ADDR_SYSTEM_CONTROL: u16 = 0x11bc;
pub const ADDR_CONFIG_SETTINGS: u16 = 0x11be;
pub const ADDR_TRACKPAD_SETTINGS: u16 = 0x11e2;
pub const ADDR_X_RESOLUTION: u16 = 0x11e6;
pub const ADDR_Y_RESOLUTION: u16 = 0x11e8;
pub const ADDR_XY_DYNAMIC_FILTER_BOTTOM_SPEED: u16 = 0x11ea;
pub const ADDR_XY_DYNAMIC_FILTER_TOP_SPEED: u16 = 0x11ec;
pub const ADDR_XY_DYNAMIC_FILTER_BOTTOM_BETA: u16 = 0x11ee;
pub const ADDR_GESTURE_ENABLE: u16 = 0x11f6;
pub const ADDR_TWO_FINGER_GESTURE_ENABLE: u16 = 0x11f8;
pub const ADDR_RX_TX_MAPPING: u16 = 0x1218;
pub const ADDR_CHANNEL_DISABLE: u16 = 0x1246;
pub const ADDR_SNAP_ENABLE: u16 = 0x129e;

pub const COORD_BLOCK_START: u16 = ADDR_RELATIVE_X;
pub const COORD_BLOCK_LENGTH: usize = 0x1c;
pub const I2C_WRITE_CHUNK_SIZE: usize = 30;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 10;
pub const DEFAULT_MOTION_INTERVAL_MS: u64 = 0;
pub const DEFAULT_CURSOR_DIVISOR: u16 = 3;
pub const DEFAULT_SCROLL_DIVISOR: u16 = 8;
pub const DEFAULT_SCROLL_LOW_SPEED_DIVISOR: u16 = 16;
pub const DEFAULT_SCROLL_LOW_SPEED_THRESHOLD: i16 = 48;
pub const DEFAULT_SCROLL_INERTIA_DIVISOR: u16 = 16;
pub const DEFAULT_SCROLL_MAX_STEP: i16 = 2;
pub const DEFAULT_SCROLL_INERTIA_MAX_STEP: i16 = 2;
pub const DEFAULT_ONE_FINGER_DRAG_HOLD_MS: u32 = 220;
pub const DEFAULT_CURSOR_INERTIA_ENABLED: bool = false;
pub const DEFAULT_DYNAMIC_SCALE_X10: u16 = 10;
pub const MIN_DYNAMIC_SCALE_X10: u16 = 2;
pub const MAX_DYNAMIC_SCALE_X10: u16 = 50;
const SCROLL_SMOOTHING_FP_SHIFT: u8 = 8;
const SCROLL_SMOOTHING_PREVIOUS_WEIGHT: i32 = 1;
const SCROLL_SMOOTHING_CURRENT_WEIGHT: i32 = 1;
const SCROLL_INERTIA_HISTORY_SIZE: usize = 12;
const SCROLL_INERTIA_INTERVAL_MS: u32 = 10;
const SCROLL_INERTIA_MAX_DURATION_MS: u32 = 700;
const SCROLL_INERTIA_DECAY_NUM: i32 = 900;
const SCROLL_INERTIA_DECAY_DEN: i32 = 1000;
const SCROLL_INERTIA_FP_SHIFT: u8 = 8;
const SCROLL_INERTIA_START_THRESHOLD: i32 = 1;
const SCROLL_INERTIA_MIN_VELOCITY: i32 = 1;
const SCROLL_INERTIA_RECENT_WINDOW_MS: u32 = 60;
const SCROLL_INERTIA_STALE_GAP_MS: u32 = 35;
const SCROLL_INERTIA_MIN_SAMPLES: usize = 2;
const SCROLL_INERTIA_MIN_AVG_SPEED: i32 = 4;
const CURSOR_INERTIA_INTERVAL_MS: u32 = 10;
const CURSOR_INERTIA_MAX_DURATION_MS: u32 = 3000;
const CURSOR_INERTIA_DECAY_NUM: i32 = 950;
const CURSOR_INERTIA_DECAY_DEN: i32 = 1000;
const CURSOR_INERTIA_START_THRESHOLD: i32 = 2;
const CURSOR_INERTIA_MIN_VELOCITY: i32 = 2;
const CURSOR_INERTIA_RECENT_WINDOW_MS: u32 = 60;
const CURSOR_INERTIA_STALE_GAP_MS: u32 = 35;
const CURSOR_INERTIA_MIN_SAMPLES: usize = 2;
const CURSOR_INERTIA_MIN_AVG_SPEED: i32 = 10;

pub const INFO_SHOW_RESET: u16 = 1 << 7;
pub const INFO_GLOBAL_TP_TOUCH: u16 = 1 << 9;
pub const INFO_TP_TOUCH_TOGGLED: u16 = 1 << 13;

pub const TP_MOVEMENT_DETECTED: u16 = 1 << 4;
pub const TP_FINGER_COUNT_MASK: u16 = 0x000f;
pub const TP_FINGER1_CONFIDENCE: u16 = 1 << 8;
pub const TP_FINGER2_CONFIDENCE: u16 = 1 << 9;

pub const TRACKPAD_SETTING_FLIP_X: u16 = 1 << 0;
pub const TRACKPAD_SETTING_FLIP_Y: u16 = 1 << 1;
pub const TRACKPAD_SETTING_SWITCH_XY: u16 = 1 << 2;

pub const SYS_CTRL_SW_RESET: u16 = 1 << 9;
pub const SYS_CTRL_ACK_RESET: u16 = 1 << 7;
pub const SYS_CTRL_ALP_RE_ATI: u16 = 1 << 6;
pub const SYS_CTRL_TP_RE_ATI: u16 = 1 << 5;
pub const CFG_TP_TOUCH_EVENT_EN: u16 = 1 << 13;
pub const CFG_TP_EVENT_EN: u16 = 1 << 10;
pub const CFG_GESTURE_EVENT_EN: u16 = 1 << 9;
pub const CFG_EVENT_MODE: u16 = 1 << 8;
pub const SFG_SINGLE_TAP: u16 = 1 << 0;
pub const TFG_TWO_TAP: u16 = 1 << 0;

pub const DEFAULT_X_RESOLUTION: u16 = 2457;
pub const DEFAULT_Y_RESOLUTION: u16 = 3072;
pub const DEFAULT_TRACKPAD_ATI_TARGET: u16 = 400;
pub const DEFAULT_DYNAMIC_FILTER_BOTTOM_SPEED: u16 = 30;
pub const DEFAULT_DYNAMIC_FILTER_TOP_SPEED: u16 = 511;
pub const DEFAULT_DYNAMIC_FILTER_BOTTOM_BETA: u8 = 20;
pub const ATI_TIMEOUT_MS: u64 = 1000;
pub const ATI_POLL_INTERVAL_MS: u64 = 10;
pub const READY_STARTUP_TIMEOUT_MS: u64 = 500;
pub const READY_CONFIG_TIMEOUT_MS: u64 = 100;
pub const READY_WRITE_TIMEOUT_MS: u64 = 200;
pub const READY_RUNTIME_TIMEOUT_MS: u64 = 20;
pub const SHOW_RESET_TIMEOUT_MS: u64 = 3000;
pub const RESET_DELAY_MS: u64 = 100;
pub const MAX_INIT_FAILURES_BEFORE_DEGRADED: u8 = 3;
pub const MAX_RUNTIME_READ_ERRORS: u8 = 8;
pub const MAX_PENDING_MOTION: i16 = 36;
const DIAGNOSTIC_NUDGE_INTERVAL_MS: u32 = 2000;
const DIAGNOSTIC_NUDGE_DELTA: i16 = 8;

const IQS9151_ALP_COMPENSATION: [u8; 26] = [
    0xb1, 0x12, 0xde, 0x12, 0xcb, 0x12, 0xb5, 0x12, 0xa1, 0x12, 0xd1, 0x12, 0xa8, 0x12, 0xb8, 0x12,
    0xb3, 0x12, 0xc6, 0x12, 0xa6, 0x12, 0xaf, 0x12, 0xad, 0x12,
];

const IQS9151_MAIN_CONFIG: [u8; 126] = [
    0x00, 0x00, 0x21, 0x4b, 0x04, 0x5f, 0x04, 0x5d, 0x04, 0x5d, 0x04, 0x5d, 0x04, 0x5d, 0x04, 0x5d,
    0x04, 0x5f, 0x04, 0x5d, 0x04, 0x5d, 0x04, 0x5d, 0x04, 0x5f, 0x04, 0x5f, 0x02, 0x5f, 0x90, 0x01,
    0x2c, 0x01, 0x32, 0x00, 0x32, 0x00, 0xe8, 0x03, 0x32, 0x14, 0x0a, 0x00, 0x32, 0x00, 0x32, 0x00,
    0x32, 0x00, 0x32, 0x00, 0x0a, 0x00, 0x0a, 0x00, 0x05, 0x00, 0x28, 0x00, 0xdc, 0x05, 0x05, 0x08,
    0x64, 0x00, 0x14, 0x00, 0x00, 0x00, 0x0e, 0x06, 0xa4, 0x00, 0xff, 0x1f, 0x00, 0xd0, 0x00, 0x00,
    0x00, 0x00, 0xfe, 0x0f, 0x1e, 0x1a, 0x08, 0x08, 0x02, 0x02, 0x32, 0x32, 0x46, 0x04, 0xb4, 0x64,
    0x28, 0x02, 0x02, 0x10, 0x07, 0x07, 0x00, 0x44, 0x40, 0x4b, 0x28, 0x0c, 0x0d, 0x03, 0x99, 0x09,
    0x00, 0x0c, 0x14, 0x00, 0xff, 0x01, 0x0a, 0x14, 0x05, 0x03, 0x14, 0x14, 0x02, 0x14,
];

const IQS9151_RX_TX_MAPPING: [u8; 46] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x21, 0x23, 0x2d,
    0x22, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const IQS9151_CHANNEL_DISABLE: [u8; 88] = [
    0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const IQS9151_SNAP_ENABLE: [u8; 88] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const CUSTOM_EVENT_PREFIX: [u8; 4] = *b"IQSP";
const CUSTOM_EVENT_POINTER_MOTION: u8 = 1;
const CUSTOM_EVENT_DYNAMIC_SCALE: u8 = 2;
const CUSTOM_EVENT_PINCH_REPORT: u8 = 3;

fn custom_event_payload(kind: u8) -> [u8; 16] {
    let mut payload = [0u8; 16];
    payload[0..4].copy_from_slice(&CUSTOM_EVENT_PREFIX);
    payload[4] = kind;
    payload
}

fn custom_event_is(payload: [u8; 16], kind: u8) -> bool {
    payload[0..4] == CUSTOM_EVENT_PREFIX && payload[4] == kind
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Iqs9151Error<E> {
    Bus(E),
    UnexpectedProductNumber(u16),
    AtiTimeout,
    ShowResetTimeout,
}

pub struct Iqs9151<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> Iqs9151<I2C>
where
    I2C: I2c,
{
    pub const fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            address: I2C_ADDRESS,
        }
    }

    pub const fn with_address(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    pub fn release(self) -> I2C {
        self.i2c
    }

    pub async fn read_product_number(&mut self) -> Result<u16, Iqs9151Error<I2C::Error>> {
        self.read_u16(ADDR_PRODUCT_NUMBER).await
    }

    pub async fn verify_product_number(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        let product_number = self.read_product_number().await?;
        if product_number != PRODUCT_NUMBER {
            return Err(Iqs9151Error::UnexpectedProductNumber(product_number));
        }
        Ok(())
    }

    pub async fn read_coordinate_frame(
        &mut self,
    ) -> Result<CoordinateFrame, Iqs9151Error<I2C::Error>> {
        let mut block = [0u8; COORD_BLOCK_LENGTH];
        self.read_block(COORD_BLOCK_START, &mut block).await?;
        Ok(CoordinateFrame::parse(&block))
    }

    pub async fn read_u16(&mut self, register: u16) -> Result<u16, Iqs9151Error<I2C::Error>> {
        let mut bytes = [0u8; 2];
        self.read_block(register, &mut bytes).await?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub async fn write_u16(
        &mut self,
        register: u16,
        value: u16,
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let register = register.to_le_bytes();
        let value = value.to_le_bytes();
        let bytes = [register[0], register[1], value[0], value[1]];
        self.i2c
            .write(self.address, &bytes)
            .await
            .map_err(Iqs9151Error::Bus)
    }

    pub async fn write_u8(
        &mut self,
        register: u16,
        value: u8,
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let register = register.to_le_bytes();
        let bytes = [register[0], register[1], value];
        self.i2c
            .write(self.address, &bytes)
            .await
            .map_err(Iqs9151Error::Bus)
    }

    async fn write_slice(
        &mut self,
        register: u16,
        bytes: &[u8],
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let register = register.to_le_bytes();
        let mut tx = [0u8; I2C_WRITE_CHUNK_SIZE + 2];
        tx[0] = register[0];
        tx[1] = register[1];
        tx[2..(2 + bytes.len())].copy_from_slice(bytes);
        self.i2c
            .write(self.address, &tx[..(2 + bytes.len())])
            .await
            .map_err(Iqs9151Error::Bus)
    }

    pub async fn update_bits_u16(
        &mut self,
        register: u16,
        mask: u16,
        value: u16,
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let current = self.read_u16(register).await?;
        self.write_u16(register, (current & !mask) | (value & mask))
            .await
    }

    pub async fn read_block(
        &mut self,
        register: u16,
        bytes: &mut [u8],
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let register = register.to_le_bytes();
        self.i2c
            .write_read(self.address, &register, bytes)
            .await
            .map_err(Iqs9151Error::Bus)
    }
}

pub trait Iqs9151Ready {
    fn wait_ready(&mut self) -> impl Future<Output = ()>;
    fn wait_ready_edge(&mut self) -> impl Future<Output = ()>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoReadyPin;

impl Iqs9151Ready for NoReadyPin {
    async fn wait_ready(&mut self) {}
    async fn wait_ready_edge(&mut self) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadyPinPolarity {
    ActiveLow,
    ActiveHigh,
}

pub struct Iqs9151ReadyPin<PIN> {
    pin: PIN,
    polarity: ReadyPinPolarity,
}

impl<PIN> Iqs9151ReadyPin<PIN> {
    pub const fn new(pin: PIN, polarity: ReadyPinPolarity) -> Self {
        Self { pin, polarity }
    }

    pub const fn active_low(pin: PIN) -> Self {
        Self::new(pin, ReadyPinPolarity::ActiveLow)
    }

    pub const fn active_high(pin: PIN) -> Self {
        Self::new(pin, ReadyPinPolarity::ActiveHigh)
    }

    pub fn release(self) -> PIN {
        self.pin
    }
}

impl<PIN> Iqs9151Ready for Iqs9151ReadyPin<PIN>
where
    PIN: Wait,
{
    async fn wait_ready(&mut self) {
        match self.polarity {
            ReadyPinPolarity::ActiveLow => {
                let _ = self.pin.wait_for_low().await;
            }
            ReadyPinPolarity::ActiveHigh => {
                let _ = self.pin.wait_for_high().await;
            }
        }
    }

    async fn wait_ready_edge(&mut self) {
        match self.polarity {
            ReadyPinPolarity::ActiveLow => {
                let _ = self.pin.wait_for_falling_edge().await;
            }
            ReadyPinPolarity::ActiveHigh => {
                let _ = self.pin.wait_for_rising_edge().await;
            }
        }
    }
}

pub struct Iqs9151InputDevice<I2C, RDY = NoReadyPin> {
    sensor: Iqs9151<I2C>,
    ready: RDY,
    side: TrackpadSide,
    motion_output: Iqs9151MotionOutput,
    initialized: bool,
    recognizer: TrackpadGestureRecognizer,
    motion_config: TrackpadMotionConfig,
    scroll_config: TrackpadScrollConfig,
    motion_remainder_x: i32,
    motion_remainder_y: i32,
    scroll_remainder_x: i32,
    scroll_remainder_y: i32,
    scroll_smooth_x_fp: i32,
    scroll_smooth_y_fp: i32,
    cursor_history: ScrollMotionHistory,
    cursor_inertia: ScrollInertiaState,
    cursor_inertia_enabled: bool,
    scroll_history: ScrollMotionHistory,
    scroll_inertia: ScrollInertiaState,
    pointer_buttons: u8,
    virtual_buttons: TrackpadButtonState,
    pending_virtual_buttons: Option<TrackpadButtonEvents>,
    pending_click: Option<TrackpadClickEvents>,
    pinch_pressed: bool,
    pending_pinch_report: Option<TrackpadPinchReport>,
    pending_motion: Option<TrackpadMotionEvent>,
    last_finger_count: u8,
    poll_interval: Duration,
    motion_interval: Option<Duration>,
    init_failure_count: u8,
    read_error_count: u8,
    degraded_mode: bool,
    diagnostic_motion_last_ms: u32,
    diagnostic_motion_sign: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameRuntime {
    previous_finger_count: u8,
    current_finger_count: u8,
    now_ms: u32,
}

impl FrameRuntime {
    const fn new(previous_finger_count: u8, current_finger_count: u8, now_ms: u32) -> Self {
        Self {
            previous_finger_count,
            current_finger_count,
            now_ms,
        }
    }
}

impl<I2C> Iqs9151InputDevice<I2C, NoReadyPin>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C, side: TrackpadSide) -> Self {
        Self::from_sensor(Iqs9151::new(i2c), side)
    }

    pub fn from_sensor(sensor: Iqs9151<I2C>, side: TrackpadSide) -> Self {
        Self::from_sensor_and_ready(sensor, NoReadyPin, side)
    }

    pub fn with_ready_pin<PIN>(
        i2c: I2C,
        ready: Iqs9151ReadyPin<PIN>,
        side: TrackpadSide,
    ) -> Iqs9151InputDevice<I2C, Iqs9151ReadyPin<PIN>>
    where
        Iqs9151ReadyPin<PIN>: Iqs9151Ready,
    {
        Iqs9151InputDevice::from_sensor_and_ready(Iqs9151::new(i2c), ready, side)
    }
}

impl<I2C, RDY> Iqs9151InputDevice<I2C, RDY>
where
    I2C: I2c,
    RDY: Iqs9151Ready,
{
    pub fn from_sensor_and_ready(sensor: Iqs9151<I2C>, ready: RDY, side: TrackpadSide) -> Self {
        Self {
            sensor,
            ready,
            side,
            motion_output: Iqs9151MotionOutput::RmkEvent,
            initialized: false,
            recognizer: TrackpadGestureRecognizer::with_defaults(),
            motion_config: TrackpadMotionConfig::default(),
            scroll_config: TrackpadScrollConfig::default(),
            motion_remainder_x: 0,
            motion_remainder_y: 0,
            scroll_remainder_x: 0,
            scroll_remainder_y: 0,
            scroll_smooth_x_fp: 0,
            scroll_smooth_y_fp: 0,
            cursor_history: ScrollMotionHistory::new(),
            cursor_inertia: ScrollInertiaState::new(),
            cursor_inertia_enabled: DEFAULT_CURSOR_INERTIA_ENABLED,
            scroll_history: ScrollMotionHistory::new(),
            scroll_inertia: ScrollInertiaState::new(),
            pointer_buttons: 0,
            virtual_buttons: TrackpadButtonState::new(side),
            pending_virtual_buttons: None,
            pending_click: None,
            pinch_pressed: false,
            pending_pinch_report: None,
            pending_motion: None,
            last_finger_count: 0,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            motion_interval: default_motion_interval(),
            init_failure_count: 0,
            read_error_count: 0,
            degraded_mode: false,
            diagnostic_motion_last_ms: 0,
            diagnostic_motion_sign: 1,
        }
    }

    pub fn release(self) -> (I2C, RDY) {
        (self.sensor.release(), self.ready)
    }

    pub fn set_poll_interval(&mut self, poll_interval: Duration) {
        self.poll_interval = poll_interval;
    }

    pub fn set_motion_interval(&mut self, motion_interval: Duration) {
        self.motion_interval = Some(motion_interval);
    }

    pub fn clear_motion_interval(&mut self) {
        self.motion_interval = None;
    }

    pub fn set_cursor_inertia_enabled(&mut self, enabled: bool) {
        self.cursor_inertia_enabled = enabled;
        if !enabled {
            self.cursor_history.reset();
            self.cursor_inertia.reset();
        }
    }

    pub fn set_gesture_config(&mut self, config: TrackpadGestureConfig) {
        self.recognizer = TrackpadGestureRecognizer::new(config);
    }

    pub fn set_motion_config(&mut self, config: TrackpadMotionConfig) {
        self.motion_config = config;
        self.motion_remainder_x = 0;
        self.motion_remainder_y = 0;
    }

    pub fn set_scroll_config(&mut self, config: TrackpadScrollConfig) {
        self.scroll_config = config;
        self.scroll_remainder_x = 0;
        self.scroll_remainder_y = 0;
        self.reset_scroll_smoothing();
    }

    pub fn set_motion_output(&mut self, output: Iqs9151MotionOutput) {
        self.motion_output = output;
    }

    pub async fn verify_product_number(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        self.sensor.verify_product_number().await
    }

    async fn initialize_sensor(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.sensor.verify_product_number().await?;

        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.software_reset().await?;

        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.ack_reset().await?;

        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.configure_sensor_settings().await?;

        self.wait_ready_for(READY_CONFIG_TIMEOUT_MS).await;
        self.apply_config_overrides().await?;

        self.wait_ready_for(READY_CONFIG_TIMEOUT_MS).await;
        self.sensor
            .update_bits_u16(
                ADDR_SYSTEM_CONTROL,
                SYS_CTRL_ALP_RE_ATI | SYS_CTRL_TP_RE_ATI,
                SYS_CTRL_ALP_RE_ATI | SYS_CTRL_TP_RE_ATI,
            )
            .await?;
        self.wait_for_ati().await?;

        self.wait_ready_for(READY_CONFIG_TIMEOUT_MS).await;
        self.sensor
            .update_bits_u16(ADDR_CONFIG_SETTINGS, CFG_EVENT_MODE, CFG_EVENT_MODE)
            .await?;

        self.init_failure_count = 0;
        self.read_error_count = 0;
        self.degraded_mode = false;
        Ok(())
    }

    async fn software_reset(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        let control = self.sensor.read_u16(ADDR_SYSTEM_CONTROL).await?;
        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.sensor
            .write_u16(ADDR_SYSTEM_CONTROL, control | SYS_CTRL_SW_RESET)
            .await?;
        let _ = self.wait_for_show_reset().await;
        Ok(())
    }

    async fn wait_for_show_reset(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        let deadline = Instant::now() + Duration::from_millis(SHOW_RESET_TIMEOUT_MS);
        loop {
            self.wait_ready_for(READY_CONFIG_TIMEOUT_MS).await;
            let info = self.sensor.read_u16(ADDR_INFO_FLAGS).await?;
            if info & INFO_SHOW_RESET != 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Iqs9151Error::ShowResetTimeout);
            }
            Timer::after(Duration::from_millis(ATI_POLL_INTERVAL_MS)).await;
        }
    }

    async fn ack_reset(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        let control = self.sensor.read_u16(ADDR_SYSTEM_CONTROL).await?;
        self.wait_ready_for(READY_STARTUP_TIMEOUT_MS).await;
        self.sensor
            .write_u16(ADDR_SYSTEM_CONTROL, control | SYS_CTRL_ACK_RESET)
            .await?;
        Timer::after(Duration::from_millis(RESET_DELAY_MS)).await;
        Ok(())
    }

    async fn configure_sensor_settings(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        self.write_sensor_chunks(ADDR_ALP_COMPENSATION, &IQS9151_ALP_COMPENSATION)
            .await?;
        self.write_sensor_chunks(ADDR_SETTINGS_MINOR, &IQS9151_MAIN_CONFIG)
            .await?;
        self.write_sensor_chunks(ADDR_RX_TX_MAPPING, &IQS9151_RX_TX_MAPPING)
            .await?;
        self.write_sensor_chunks(ADDR_CHANNEL_DISABLE, &IQS9151_CHANNEL_DISABLE)
            .await?;
        self.write_sensor_chunks(ADDR_SNAP_ENABLE, &IQS9151_SNAP_ENABLE)
            .await
    }

    async fn apply_config_overrides(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        self.sensor
            .update_bits_u16(
                ADDR_TRACKPAD_SETTINGS,
                TRACKPAD_SETTING_FLIP_X | TRACKPAD_SETTING_FLIP_Y | TRACKPAD_SETTING_SWITCH_XY,
                0,
            )
            .await?;
        self.sensor
            .write_u16(ADDR_X_RESOLUTION, DEFAULT_X_RESOLUTION)
            .await?;
        self.sensor
            .write_u16(ADDR_Y_RESOLUTION, DEFAULT_Y_RESOLUTION)
            .await?;
        self.sensor
            .write_u16(ADDR_TRACKPAD_ATI_TARGET, DEFAULT_TRACKPAD_ATI_TARGET)
            .await?;
        self.sensor
            .write_u16(
                ADDR_XY_DYNAMIC_FILTER_BOTTOM_SPEED,
                DEFAULT_DYNAMIC_FILTER_BOTTOM_SPEED,
            )
            .await?;
        self.sensor
            .write_u16(
                ADDR_XY_DYNAMIC_FILTER_TOP_SPEED,
                DEFAULT_DYNAMIC_FILTER_TOP_SPEED,
            )
            .await?;
        self.sensor
            .write_u8(
                ADDR_XY_DYNAMIC_FILTER_BOTTOM_BETA,
                DEFAULT_DYNAMIC_FILTER_BOTTOM_BETA,
            )
            .await
    }

    async fn write_sensor_chunks(
        &mut self,
        start_register: u16,
        bytes: &[u8],
    ) -> Result<(), Iqs9151Error<I2C::Error>> {
        let mut offset = 0usize;
        while offset < bytes.len() {
            let remaining = bytes.len() - offset;
            let chunk_len = if remaining > I2C_WRITE_CHUNK_SIZE {
                I2C_WRITE_CHUNK_SIZE
            } else {
                remaining
            };
            self.wait_ready_for(READY_WRITE_TIMEOUT_MS).await;
            self.sensor
                .write_slice(
                    start_register + offset as u16,
                    &bytes[offset..(offset + chunk_len)],
                )
                .await?;
            offset += chunk_len;
        }
        Ok(())
    }

    async fn wait_ready_for(&mut self, timeout_ms: u64) -> bool {
        with_timeout(Duration::from_millis(timeout_ms), self.ready.wait_ready())
            .await
            .is_ok()
    }

    async fn wait_for_ati(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        let deadline = Instant::now() + Duration::from_millis(ATI_TIMEOUT_MS);
        loop {
            self.wait_ready_for(READY_CONFIG_TIMEOUT_MS).await;
            let control = self.sensor.read_u16(ADDR_SYSTEM_CONTROL).await?;
            if control & (SYS_CTRL_ALP_RE_ATI | SYS_CTRL_TP_RE_ATI) == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Iqs9151Error::AtiTimeout);
            }
            Timer::after(Duration::from_millis(ATI_POLL_INTERVAL_MS)).await;
        }
    }

    fn motion_from_frame(&mut self, frame: CoordinateFrame) -> Option<TrackpadMotionEvent> {
        if !cursor_motion_detected(frame) {
            return None;
        }

        self.motion_config.motion_event(
            self.side,
            frame.relative_x,
            frame.relative_y,
            &mut self.motion_remainder_x,
            &mut self.motion_remainder_y,
        )
    }

    fn scroll_from_delta(&mut self, delta: TrackpadScrollDelta) -> Option<TrackpadMotionEvent> {
        self.scroll_config.scroll_event(
            self.side,
            delta.x,
            delta.y,
            &mut self.scroll_remainder_x,
            &mut self.scroll_remainder_y,
        )
    }

    fn handle_scroll_delta(&mut self, delta: TrackpadScrollDelta) -> Option<Event> {
        self.scroll_from_delta(delta)
            .and_then(|scroll| self.handle_motion(scroll))
    }

    fn handle_inertia_scroll_delta(&mut self, delta: TrackpadScrollDelta) -> Option<Event> {
        self.scroll_config
            .inertia_scroll_event(
                self.side,
                delta.x,
                delta.y,
                &mut self.scroll_remainder_x,
                &mut self.scroll_remainder_y,
            )
            .and_then(|scroll| self.handle_motion(scroll))
    }

    fn pinch_motion_from_wheel(&mut self, wheel: i16) -> Option<TrackpadMotionEvent> {
        pinch_wheel_to_motion(self.side, wheel)
    }

    fn pinch_report_event(&mut self, started: bool, ended: bool, wheel: i16) -> Option<Event> {
        if started {
            self.reset_scroll_runtime();
        }

        let motion = self
            .pinch_motion_from_wheel(wheel)
            .unwrap_or_else(|| TrackpadMotionEvent::scroll(self.side, 0, 0));

        if !started && !ended && motion.is_empty() {
            return None;
        }

        if started {
            self.pinch_pressed = true;
        }
        if ended {
            self.pinch_pressed = false;
        }

        Some(Event::Custom(
            TrackpadPinchReport::new(self.side, started, ended, motion).encode(),
        ))
    }

    fn release_pinch_report(&mut self) {
        if !self.pinch_pressed {
            return;
        }

        self.pinch_pressed = false;
        self.pending_pinch_report = Some(TrackpadPinchReport::new(
            self.side,
            false,
            true,
            TrackpadMotionEvent::scroll(self.side, 0, 0),
        ));
    }

    fn record_cursor_motion(&mut self, motion: TrackpadMotionEvent, now_ms: u32) {
        if motion.x == 0 && motion.y == 0 {
            return;
        }

        self.cursor_inertia.reset();
        self.cursor_history.push(
            TrackpadScrollDelta {
                x: motion.x,
                y: motion.y,
            },
            now_ms,
        );
    }

    fn start_cursor_inertia(&mut self, now_ms: u32) {
        if !self.cursor_inertia_enabled {
            self.cursor_history.reset();
            self.cursor_inertia.reset();
            return;
        }

        self.motion_remainder_x = 0;
        self.motion_remainder_y = 0;
        if let Some(seed) = self.cursor_history.seed_with(now_ms, CURSOR_INERTIA_CONFIG) {
            self.cursor_inertia
                .start_with(seed, now_ms, CURSOR_INERTIA_CONFIG);
        }
        self.cursor_history.reset();
    }

    fn flush_cursor_inertia(&mut self) -> Option<Event> {
        let now_ms = Instant::now().as_millis() as u32;
        self.cursor_inertia
            .step_with(now_ms, CURSOR_INERTIA_CONFIG)
            .and_then(|delta| {
                self.handle_motion(TrackpadMotionEvent::cursor(self.side, delta.x, delta.y))
            })
    }

    fn reset_cursor_runtime(&mut self) {
        self.motion_remainder_x = 0;
        self.motion_remainder_y = 0;
        self.cursor_history.reset();
        self.cursor_inertia.reset();
        self.last_finger_count = 0;
    }

    fn record_active_scroll(&mut self, delta: TrackpadScrollDelta, now_ms: u32) -> Option<Event> {
        self.scroll_inertia.reset();
        self.cursor_history.reset();
        self.cursor_inertia.reset();
        if delta.x == 0 && delta.y == 0 {
            self.reset_scroll_smoothing();
            return None;
        }

        let delta = self.smooth_scroll_delta(delta);
        self.scroll_history.push(delta, now_ms);
        self.handle_scroll_delta(delta)
    }

    fn start_scroll_inertia(&mut self, now_ms: u32) {
        self.scroll_remainder_x = 0;
        self.scroll_remainder_y = 0;
        self.reset_scroll_smoothing();
        if let Some(seed) = self.scroll_history.seed(now_ms) {
            self.scroll_inertia.start(seed, now_ms);
        }
        self.scroll_history.reset();
    }

    fn flush_scroll_inertia(&mut self) -> Option<Event> {
        let now_ms = Instant::now().as_millis() as u32;
        self.scroll_inertia
            .step(now_ms)
            .and_then(|delta| self.handle_inertia_scroll_delta(delta))
    }

    fn reset_scroll_runtime(&mut self) {
        self.scroll_remainder_x = 0;
        self.scroll_remainder_y = 0;
        self.reset_scroll_smoothing();
        self.scroll_history.reset();
        self.scroll_inertia.reset();
    }

    fn reset_sensor_runtime(&mut self, clear_pending_motion: bool) {
        if clear_pending_motion {
            self.pending_motion = None;
        }
        self.release_pinch_report();
        self.release_pointer_buttons();
        self.release_virtual_buttons();
        self.recognizer.reset();
        self.reset_cursor_runtime();
        self.reset_scroll_runtime();
    }

    fn smooth_scroll_delta(&mut self, delta: TrackpadScrollDelta) -> TrackpadScrollDelta {
        self.scroll_smooth_x_fp = smooth_scroll_axis(self.scroll_smooth_x_fp, delta.x);
        self.scroll_smooth_y_fp = smooth_scroll_axis(self.scroll_smooth_y_fp, delta.y);

        TrackpadScrollDelta {
            x: fixed_point_to_i16_rounded(self.scroll_smooth_x_fp, SCROLL_SMOOTHING_FP_SHIFT),
            y: fixed_point_to_i16_rounded(self.scroll_smooth_y_fp, SCROLL_SMOOTHING_FP_SHIFT),
        }
    }

    fn reset_scroll_smoothing(&mut self) {
        self.scroll_smooth_x_fp = 0;
        self.scroll_smooth_y_fp = 0;
    }

    fn handle_trackpad_button(&mut self, button: TrackpadButton, pressed: bool) -> Option<Event> {
        if button.mouse_button_mask().is_none() {
            return self.handle_virtual_button(button, pressed);
        }

        self.handle_pointer_button(button, pressed)
    }

    fn handle_pointer_button(&mut self, button: TrackpadButton, pressed: bool) -> Option<Event> {
        let Some(mask) = button.mouse_button_mask() else {
            return None;
        };

        if pressed {
            self.pointer_buttons |= mask;
        } else {
            self.pointer_buttons &= !mask;
        }

        self.handle_motion(TrackpadMotionEvent::button_state(
            self.side,
            self.pointer_buttons,
        ))
    }

    fn handle_virtual_button(&mut self, button: TrackpadButton, pressed: bool) -> Option<Event> {
        let next_bits = if pressed {
            self.virtual_buttons.pressed_bits() | button.bit()
        } else {
            self.virtual_buttons.pressed_bits() & !button.bit()
        };
        let mut events = self.virtual_buttons.update(next_bits);
        let next_event = events.next();
        self.pending_virtual_buttons = Some(events);
        next_event.map(TrackpadButtonEvent::into_rmk_event)
    }

    fn flush_pending_virtual_buttons(&mut self) -> Option<Event> {
        if let Some(events) = self.pending_virtual_buttons.as_mut() {
            if let Some(event) = events.next() {
                return Some(event.into_rmk_event());
            }
            self.pending_virtual_buttons = None;
        }
        None
    }

    fn release_pointer_buttons(&mut self) {
        if self.pointer_buttons == 0 {
            return;
        }

        self.pointer_buttons = 0;
        let motion = TrackpadMotionEvent::button_state(self.side, 0);
        if self.motion_output == Iqs9151MotionOutput::RmkEvent {
            self.pending_motion = Some(motion);
        } else {
            self.send_or_queue_motion(motion);
        }
    }

    fn release_virtual_buttons(&mut self) {
        if self.virtual_buttons.pressed_bits() == 0 {
            return;
        }

        self.pending_virtual_buttons = Some(self.virtual_buttons.update(0));
    }

    fn finish_frame_runtime(&mut self, runtime: FrameRuntime, allow_cursor_inertia: bool) {
        if runtime.current_finger_count == 0 {
            if allow_cursor_inertia
                && runtime.previous_finger_count == 1
                && !self.recognizer.cursor_suppressed()
            {
                self.start_cursor_inertia(runtime.now_ms);
            }
            self.motion_remainder_x = 0;
            self.motion_remainder_y = 0;
            if !self.scroll_inertia.active {
                self.scroll_remainder_x = 0;
                self.scroll_remainder_y = 0;
            }
        } else if runtime.current_finger_count >= 2 {
            self.cursor_history.reset();
            self.cursor_inertia.reset();
        }

        self.last_finger_count = runtime.current_finger_count;
    }

    fn finish_frame_with_event(
        &mut self,
        event: Event,
        runtime: FrameRuntime,
        allow_cursor_inertia: bool,
    ) -> Event {
        self.finish_frame_runtime(runtime, allow_cursor_inertia);
        event
    }

    fn handle_motion(&mut self, motion: TrackpadMotionEvent) -> Option<Event> {
        let motion = motion.with_button_state(self.pointer_buttons).capped();
        match self.motion_output {
            Iqs9151MotionOutput::RmkEvent => Some(motion.into_rmk_event()),
            Iqs9151MotionOutput::HidReport | Iqs9151MotionOutput::SplitEvent => {
                self.send_or_queue_motion(motion);
                None
            }
        }
    }

    fn flush_pending_motion(&mut self) {
        let Some(motion) = self.pending_motion else {
            return;
        };

        let sent = self.motion_output.try_send_motion(motion);
        if sent {
            self.pending_motion = None;
        }
    }

    fn send_or_queue_motion(&mut self, motion: TrackpadMotionEvent) {
        if !self.motion_output.try_send_motion(motion) {
            self.queue_pending_motion(motion);
        }
    }

    fn queue_pending_motion(&mut self, motion: TrackpadMotionEvent) {
        self.pending_motion = Some(match self.pending_motion {
            Some(pending) => pending.merge(motion),
            None => motion.capped(),
        });
    }

    fn send_diagnostic_nudge(&mut self) {
        let now_ms = Instant::now().as_millis() as u32;
        if now_ms.wrapping_sub(self.diagnostic_motion_last_ms) < DIAGNOSTIC_NUDGE_INTERVAL_MS {
            return;
        }
        self.diagnostic_motion_last_ms = now_ms;

        let motion = TrackpadMotionEvent {
            side: self.side,
            x: DIAGNOSTIC_NUDGE_DELTA.saturating_mul(self.diagnostic_motion_sign),
            y: 0,
            wheel: 0,
            pan: 0,
            buttons: 0,
            button_state_valid: false,
        };
        self.diagnostic_motion_sign = -self.diagnostic_motion_sign;
        let _ = self.handle_motion(motion);
    }

    async fn wait_motion_interval(&mut self) {
        if let Some(motion_interval) = self.motion_interval {
            Timer::after(motion_interval).await;
        }
    }
}

impl<I2C, RDY> InputDevice for Iqs9151InputDevice<I2C, RDY>
where
    I2C: I2c,
    RDY: Iqs9151Ready,
{
    async fn read_event(&mut self) -> Event {
        loop {
            if !self.initialized {
                match self.initialize_sensor().await {
                    Ok(()) => {
                        self.initialized = true;
                    }
                    Err(_) => {
                        self.init_failure_count = self.init_failure_count.saturating_add(1);
                        self.pending_motion = None;
                        self.release_pinch_report();
                        self.release_pointer_buttons();
                        self.release_virtual_buttons();
                        self.recognizer.reset();
                        self.reset_cursor_runtime();
                        self.reset_scroll_runtime();
                        self.send_diagnostic_nudge();
                        if self.init_failure_count >= MAX_INIT_FAILURES_BEFORE_DEGRADED {
                            self.initialized = true;
                            self.degraded_mode = true;
                            self.read_error_count = 0;
                            continue;
                        }
                        Timer::after(self.poll_interval).await;
                        continue;
                    }
                }
            }

            if let Some(event) = self.flush_pending_virtual_buttons() {
                return event;
            }

            if let Some(report) = self.pending_pinch_report.take() {
                return Event::Custom(report.encode());
            }

            if let Some(events) = self.pending_click.as_mut() {
                if let Some(event) = events.next() {
                    return event.into_rmk_event();
                }
                self.pending_click = None;
            }

            self.flush_pending_motion();
            if let Some(event) = self.flush_cursor_inertia() {
                return event;
            }
            if let Some(event) = self.flush_scroll_inertia() {
                return event;
            }
            self.wait_ready_for(READY_RUNTIME_TIMEOUT_MS).await;

            match self.sensor.read_coordinate_frame().await {
                Ok(frame) => {
                    self.read_error_count = 0;
                    if frame.show_reset() {
                        self.reset_sensor_runtime(true);
                        self.initialized = false;
                        self.degraded_mode = false;
                        continue;
                    }

                    let runtime = FrameRuntime::new(
                        self.last_finger_count,
                        frame.finger_count(),
                        Instant::now().as_millis() as u32,
                    );
                    if runtime.current_finger_count > 0 {
                        self.scroll_inertia.reset();
                        self.cursor_inertia.reset();
                    }
                    if runtime.previous_finger_count == 0 && runtime.current_finger_count == 1 {
                        self.cursor_history.reset();
                    }
                    if let Some(gesture) = self.recognizer.update(frame, runtime.now_ms) {
                        match gesture {
                            TrackpadGestureEvent::Button { button, pressed } => {
                                if button == TrackpadButton::Pinch {
                                    if let Some(event) = self.pinch_report_event(false, !pressed, 0)
                                    {
                                        return self.finish_frame_with_event(event, runtime, false);
                                    }
                                    self.finish_frame_runtime(runtime, false);
                                    continue;
                                }
                                if let Some(event) = self.handle_trackpad_button(button, pressed) {
                                    return self.finish_frame_with_event(event, runtime, false);
                                }
                            }
                            TrackpadGestureEvent::Click(button) => {
                                self.pending_click =
                                    Some(TrackpadClickEvents::new(self.side, button));
                            }
                            TrackpadGestureEvent::ReleaseAndClick(button) => {
                                self.pending_click =
                                    Some(TrackpadClickEvents::new(self.side, button));
                                if let Some(event) = self.handle_trackpad_button(button, false) {
                                    return self.finish_frame_with_event(event, runtime, false);
                                }
                            }
                            TrackpadGestureEvent::Scroll(delta) => {
                                if let Some(event) =
                                    self.record_active_scroll(delta, runtime.now_ms)
                                {
                                    return self.finish_frame_with_event(event, runtime, false);
                                }
                            }
                            TrackpadGestureEvent::ScrollEnded => {
                                self.start_scroll_inertia(runtime.now_ms)
                            }
                            TrackpadGestureEvent::PinchStarted(wheel) => {
                                self.scroll_inertia.reset();
                                self.scroll_history.reset();
                                if let Some(event) = self.pinch_report_event(true, false, wheel) {
                                    return self.finish_frame_with_event(event, runtime, false);
                                }
                            }
                            TrackpadGestureEvent::PinchWheel(wheel) => {
                                self.scroll_inertia.reset();
                                self.scroll_history.reset();
                                if let Some(event) = self.pinch_report_event(false, false, wheel) {
                                    return self.finish_frame_with_event(event, runtime, false);
                                }
                            }
                            TrackpadGestureEvent::DeferredHoldPending => {}
                        }
                        self.finish_frame_runtime(runtime, false);
                        continue;
                    }

                    if !self.recognizer.cursor_suppressed() {
                        if let Some(motion) = self.motion_from_frame(frame) {
                            self.record_cursor_motion(motion, runtime.now_ms);
                            if let Some(event) = self.handle_motion(motion) {
                                return self.finish_frame_with_event(event, runtime, true);
                            }
                            if self.motion_interval.is_some() {
                                self.wait_motion_interval().await;
                            }
                            self.finish_frame_runtime(runtime, true);
                            continue;
                        }
                    }

                    self.finish_frame_runtime(runtime, true);
                }
                Err(_) => {
                    self.read_error_count = self.read_error_count.saturating_add(1);
                    self.reset_sensor_runtime(false);
                    if self.read_error_count >= MAX_RUNTIME_READ_ERRORS {
                        self.initialized = false;
                        self.degraded_mode = false;
                        self.read_error_count = 0;
                    } else if self.degraded_mode {
                        self.send_diagnostic_nudge();
                    }
                }
            }

            Timer::after(self.poll_interval).await;
        }
    }
}

pub struct Iqs9151KeyboardController<DEVICE> {
    device: DEVICE,
    target: Iqs9151ControllerTarget,
}

impl<DEVICE> Iqs9151KeyboardController<DEVICE> {
    pub const fn new_central(device: DEVICE) -> Self {
        Self {
            device,
            target: Iqs9151ControllerTarget::HidReport,
        }
    }

    pub const fn new_peripheral(device: DEVICE) -> Self {
        Self {
            device,
            target: Iqs9151ControllerTarget::SplitEvent,
        }
    }

    pub fn release(self) -> DEVICE {
        self.device
    }
}

impl<DEVICE> Controller for Iqs9151KeyboardController<DEVICE>
where
    DEVICE: InputDevice,
{
    type Event = Event;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            Event::Key(key_event) => KEY_EVENT_CHANNEL.send(key_event).await,
            Event::Custom(payload) => process_controller_custom_event(payload, self.target).await,
            _ => {}
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.device.read_event().await
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Iqs9151ControllerTarget {
    HidReport,
    SplitEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Iqs9151MotionOutput {
    RmkEvent,
    HidReport,
    SplitEvent,
}

impl Iqs9151MotionOutput {
    fn try_send_motion(self, motion: TrackpadMotionEvent) -> bool {
        match self {
            Self::RmkEvent => false,
            Self::HidReport => send_mouse_motion_reports(motion),
            Self::SplitEvent => send_split_motion_event(motion),
        }
    }
}

pub struct Iqs9151SplitEventController;

impl Iqs9151SplitEventController {
    pub const fn new() -> Self {
        Self
    }
}

impl Controller for Iqs9151SplitEventController {
    type Event = Event;

    async fn process_event(&mut self, event: Self::Event) {
        if let Event::Custom(payload) = event {
            process_controller_custom_event(payload, Iqs9151ControllerTarget::HidReport).await;
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        EVENT_CHANNEL.receive().await
    }
}

static LEFT_TRACKPAD_MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);
static RIGHT_TRACKPAD_MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);

async fn process_controller_custom_event(payload: [u8; 16], target: Iqs9151ControllerTarget) {
    if let Some(scale) = TrackpadDynamicScaleEvent::decode(payload) {
        scale.apply();
        return;
    }

    match target {
        Iqs9151ControllerTarget::HidReport => {
            if let Some(pinch) = TrackpadPinchReport::decode(payload) {
                process_pinch_report(pinch).await;
            } else if let Some(motion) = TrackpadMotionEvent::decode(payload) {
                send_mouse_motion_reports(motion);
            }
        }
        Iqs9151ControllerTarget::SplitEvent => {
            if TrackpadPinchReport::decode(payload).is_some() {
                send_split_custom_event(payload);
            } else if let Some(motion) = TrackpadMotionEvent::decode(payload) {
                send_split_motion_event(motion);
            }
        }
    }
}

async fn process_pinch_report(report: TrackpadPinchReport) {
    let position = trackpad_button_position(report.side, TrackpadButton::Pinch);

    if report.started {
        KEY_EVENT_CHANNEL
            .send(KeyboardEvent::key(position.row, position.col, true))
            .await;
        Timer::after(Duration::from_millis(PINCH_KEY_SETTLE_MS as u64)).await;
    }

    if !report.motion.is_empty() {
        send_mouse_motion_reports(report.motion);
    }

    if report.ended {
        KEY_EVENT_CHANNEL
            .send(KeyboardEvent::key(position.row, position.col, false))
            .await;
    }
}

fn send_mouse_motion_reports(mut motion: TrackpadMotionEvent) -> bool {
    motion = apply_dynamic_motion_scale(motion).capped();
    if motion.is_empty() {
        return true;
    }

    if motion.button_state_valid {
        match motion.side {
            TrackpadSide::Left => {
                LEFT_TRACKPAD_MOUSE_BUTTONS.store(motion.buttons, Ordering::Relaxed);
            }
            TrackpadSide::Right => {
                RIGHT_TRACKPAD_MOUSE_BUTTONS.store(motion.buttons, Ordering::Relaxed);
            }
        }
    }
    let buttons = LEFT_TRACKPAD_MOUSE_BUTTONS.load(Ordering::Relaxed)
        | RIGHT_TRACKPAD_MOUSE_BUTTONS.load(Ordering::Relaxed);

    let report = MouseReport {
        buttons,
        x: clamp_i16_to_i8(motion.x),
        y: clamp_i16_to_i8(motion.y),
        wheel: clamp_i16_to_i8(motion.wheel),
        pan: clamp_i16_to_i8(motion.pan),
    };

    KEYBOARD_REPORT_CHANNEL
        .try_send(Report::MouseReport(report))
        .is_ok()
}

fn send_split_motion_event(motion: TrackpadMotionEvent) -> bool {
    let motion = motion.capped();
    if motion.is_empty() {
        return true;
    }
    send_split_custom_event(motion.encode())
}

fn send_split_custom_event(payload: [u8; 16]) -> bool {
    if EVENT_CHANNEL.is_full() {
        let _ = EVENT_CHANNEL.try_receive();
    }
    EVENT_CHANNEL.try_send(Event::Custom(payload)).is_ok()
}

fn pinch_wheel_to_motion(side: TrackpadSide, wheel: i16) -> Option<TrackpadMotionEvent> {
    let wheel = clamp_scroll_step(wheel.saturating_neg(), PINCH_WHEEL_MAX_STEP);
    if wheel == 0 {
        None
    } else {
        Some(TrackpadMotionEvent::scroll(side, wheel, 0))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadSide {
    Left,
    Right,
}

impl TrackpadSide {
    const fn to_wire(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }

    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Left),
            1 => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadDynamicScaleGroup {
    Cursor,
    Scroll,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadDynamicScaleAction {
    Increment,
    Decrement,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadDynamicScaleEvent {
    pub group: TrackpadDynamicScaleGroup,
    pub action: TrackpadDynamicScaleAction,
}

impl TrackpadDynamicScaleEvent {
    pub const fn new(group: TrackpadDynamicScaleGroup, action: TrackpadDynamicScaleAction) -> Self {
        Self { group, action }
    }

    pub fn decode(payload: [u8; 16]) -> Option<Self> {
        if !custom_event_is(payload, CUSTOM_EVENT_DYNAMIC_SCALE) {
            return None;
        }

        let group = match payload[5] {
            0 => TrackpadDynamicScaleGroup::Cursor,
            1 => TrackpadDynamicScaleGroup::Scroll,
            2 => TrackpadDynamicScaleGroup::All,
            _ => return None,
        };
        let action = match payload[6] {
            1 => TrackpadDynamicScaleAction::Increment,
            2 => TrackpadDynamicScaleAction::Decrement,
            3 => TrackpadDynamicScaleAction::Reset,
            _ => return None,
        };

        Some(Self { group, action })
    }

    pub fn apply(self) {
        match self.action {
            TrackpadDynamicScaleAction::Increment => {
                trackpad_dynamic_scale_step(self.group, 1);
            }
            TrackpadDynamicScaleAction::Decrement => {
                trackpad_dynamic_scale_step(self.group, -1);
            }
            TrackpadDynamicScaleAction::Reset => {
                trackpad_dynamic_scale_reset(self.group);
            }
        }
    }
}

static TRACKPAD_CURSOR_SCALE_X10: AtomicU16 = AtomicU16::new(DEFAULT_DYNAMIC_SCALE_X10);
static TRACKPAD_SCROLL_SCALE_X10: AtomicU16 = AtomicU16::new(DEFAULT_DYNAMIC_SCALE_X10);

static LEFT_CURSOR_X_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static LEFT_CURSOR_Y_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static RIGHT_CURSOR_X_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static RIGHT_CURSOR_Y_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static LEFT_SCROLL_WHEEL_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static LEFT_SCROLL_PAN_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static RIGHT_SCROLL_WHEEL_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);
static RIGHT_SCROLL_PAN_DYNAMIC_REMAINDER: AtomicI32 = AtomicI32::new(0);

pub fn trackpad_dynamic_scale_x10(group: TrackpadDynamicScaleGroup) -> u16 {
    match group {
        TrackpadDynamicScaleGroup::Cursor => TRACKPAD_CURSOR_SCALE_X10.load(Ordering::Relaxed),
        TrackpadDynamicScaleGroup::Scroll => TRACKPAD_SCROLL_SCALE_X10.load(Ordering::Relaxed),
        TrackpadDynamicScaleGroup::All => DEFAULT_DYNAMIC_SCALE_X10,
    }
}

pub fn trackpad_dynamic_scale_step(group: TrackpadDynamicScaleGroup, delta: i16) {
    match group {
        TrackpadDynamicScaleGroup::Cursor => step_dynamic_scale(&TRACKPAD_CURSOR_SCALE_X10, delta),
        TrackpadDynamicScaleGroup::Scroll => step_dynamic_scale(&TRACKPAD_SCROLL_SCALE_X10, delta),
        TrackpadDynamicScaleGroup::All => {
            step_dynamic_scale(&TRACKPAD_CURSOR_SCALE_X10, delta);
            step_dynamic_scale(&TRACKPAD_SCROLL_SCALE_X10, delta);
        }
    }
    reset_dynamic_scale_remainders();
}

pub fn trackpad_dynamic_scale_reset(group: TrackpadDynamicScaleGroup) {
    match group {
        TrackpadDynamicScaleGroup::Cursor => {
            TRACKPAD_CURSOR_SCALE_X10.store(DEFAULT_DYNAMIC_SCALE_X10, Ordering::Relaxed);
        }
        TrackpadDynamicScaleGroup::Scroll => {
            TRACKPAD_SCROLL_SCALE_X10.store(DEFAULT_DYNAMIC_SCALE_X10, Ordering::Relaxed);
        }
        TrackpadDynamicScaleGroup::All => {
            TRACKPAD_CURSOR_SCALE_X10.store(DEFAULT_DYNAMIC_SCALE_X10, Ordering::Relaxed);
            TRACKPAD_SCROLL_SCALE_X10.store(DEFAULT_DYNAMIC_SCALE_X10, Ordering::Relaxed);
        }
    }
    reset_dynamic_scale_remainders();
}

fn step_dynamic_scale(scale: &AtomicU16, delta: i16) {
    let current = i32::from(scale.load(Ordering::Relaxed));
    let next = (current + i32::from(delta)).clamp(
        i32::from(MIN_DYNAMIC_SCALE_X10),
        i32::from(MAX_DYNAMIC_SCALE_X10),
    );
    scale.store(next as u16, Ordering::Relaxed);
}

fn reset_dynamic_scale_remainders() {
    for remainder in [
        &LEFT_CURSOR_X_DYNAMIC_REMAINDER,
        &LEFT_CURSOR_Y_DYNAMIC_REMAINDER,
        &RIGHT_CURSOR_X_DYNAMIC_REMAINDER,
        &RIGHT_CURSOR_Y_DYNAMIC_REMAINDER,
        &LEFT_SCROLL_WHEEL_DYNAMIC_REMAINDER,
        &LEFT_SCROLL_PAN_DYNAMIC_REMAINDER,
        &RIGHT_SCROLL_WHEEL_DYNAMIC_REMAINDER,
        &RIGHT_SCROLL_PAN_DYNAMIC_REMAINDER,
    ] {
        remainder.store(0, Ordering::Relaxed);
    }
}

fn apply_dynamic_motion_scale(mut motion: TrackpadMotionEvent) -> TrackpadMotionEvent {
    if motion.x != 0 || motion.y != 0 {
        let scale = TRACKPAD_CURSOR_SCALE_X10.load(Ordering::Relaxed);
        motion.x = dynamic_scale_axis(
            motion.x,
            scale,
            dynamic_cursor_remainder(motion.side, DynamicAxis::X),
        );
        motion.y = dynamic_scale_axis(
            motion.y,
            scale,
            dynamic_cursor_remainder(motion.side, DynamicAxis::Y),
        );
    }

    if motion.wheel != 0 || motion.pan != 0 {
        let scale = TRACKPAD_SCROLL_SCALE_X10.load(Ordering::Relaxed);
        motion.wheel = dynamic_scale_axis(
            motion.wheel,
            scale,
            dynamic_scroll_remainder(motion.side, DynamicScrollAxis::Wheel),
        );
        motion.pan = dynamic_scale_axis(
            motion.pan,
            scale,
            dynamic_scroll_remainder(motion.side, DynamicScrollAxis::Pan),
        );
    }

    motion
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DynamicAxis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DynamicScrollAxis {
    Wheel,
    Pan,
}

fn dynamic_cursor_remainder(side: TrackpadSide, axis: DynamicAxis) -> &'static AtomicI32 {
    match (side, axis) {
        (TrackpadSide::Left, DynamicAxis::X) => &LEFT_CURSOR_X_DYNAMIC_REMAINDER,
        (TrackpadSide::Left, DynamicAxis::Y) => &LEFT_CURSOR_Y_DYNAMIC_REMAINDER,
        (TrackpadSide::Right, DynamicAxis::X) => &RIGHT_CURSOR_X_DYNAMIC_REMAINDER,
        (TrackpadSide::Right, DynamicAxis::Y) => &RIGHT_CURSOR_Y_DYNAMIC_REMAINDER,
    }
}

fn dynamic_scroll_remainder(side: TrackpadSide, axis: DynamicScrollAxis) -> &'static AtomicI32 {
    match (side, axis) {
        (TrackpadSide::Left, DynamicScrollAxis::Wheel) => &LEFT_SCROLL_WHEEL_DYNAMIC_REMAINDER,
        (TrackpadSide::Left, DynamicScrollAxis::Pan) => &LEFT_SCROLL_PAN_DYNAMIC_REMAINDER,
        (TrackpadSide::Right, DynamicScrollAxis::Wheel) => &RIGHT_SCROLL_WHEEL_DYNAMIC_REMAINDER,
        (TrackpadSide::Right, DynamicScrollAxis::Pan) => &RIGHT_SCROLL_PAN_DYNAMIC_REMAINDER,
    }
}

fn dynamic_scale_axis(value: i16, scale_x10: u16, remainder: &AtomicI32) -> i16 {
    let value_mul = i32::from(value)
        .saturating_mul(i32::from(scale_x10))
        .saturating_add(remainder.load(Ordering::Relaxed));
    let scaled = value_mul / 10;
    remainder.store(
        value_mul.saturating_sub(scaled.saturating_mul(10)),
        Ordering::Relaxed,
    );
    clamp_i32_to_i16(scaled)
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

    pub const fn input_btn_code(self) -> u8 {
        match self {
            Self::LeftClick => 0,
            Self::RightClick => 1,
            Self::MiddleClick => 2,
            Self::GestureLeft => 3,
            Self::GestureRight => 4,
            Self::GestureUp => 5,
            Self::GestureDown => 6,
            Self::Pinch => 7,
        }
    }

    pub const fn bit(self) -> u8 {
        1 << self.input_btn_code()
    }

    pub const fn mouse_button_mask(self) -> Option<u8> {
        match self {
            Self::LeftClick => Some(1 << 0),
            Self::RightClick => Some(1 << 1),
            Self::MiddleClick => Some(1 << 2),
            Self::GestureLeft
            | Self::GestureRight
            | Self::GestureUp
            | Self::GestureDown
            | Self::Pinch => None,
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

impl TrackpadButtonEvent {
    pub fn into_rmk_event(self) -> Event {
        Event::Key(KeyboardEvent::key(
            self.position.row,
            self.position.col,
            self.pressed,
        ))
    }
}

const LEFT_TRACKPAD_BUTTON_POSITIONS: [VirtualKeyPosition; 8] = [
    VirtualKeyPosition { row: 5, col: 0 },
    VirtualKeyPosition { row: 5, col: 1 },
    VirtualKeyPosition { row: 5, col: 2 },
    VirtualKeyPosition { row: 6, col: 0 },
    VirtualKeyPosition { row: 6, col: 1 },
    VirtualKeyPosition { row: 6, col: 2 },
    VirtualKeyPosition { row: 6, col: 3 },
    VirtualKeyPosition { row: 6, col: 4 },
];

const RIGHT_TRACKPAD_BUTTON_POSITIONS: [VirtualKeyPosition; 8] = [
    VirtualKeyPosition { row: 5, col: 9 },
    VirtualKeyPosition { row: 5, col: 10 },
    VirtualKeyPosition { row: 5, col: 11 },
    VirtualKeyPosition { row: 6, col: 7 },
    VirtualKeyPosition { row: 6, col: 8 },
    VirtualKeyPosition { row: 6, col: 9 },
    VirtualKeyPosition { row: 6, col: 10 },
    VirtualKeyPosition { row: 6, col: 11 },
];

pub const fn trackpad_button_position(
    side: TrackpadSide,
    button: TrackpadButton,
) -> VirtualKeyPosition {
    let index = button.input_btn_code() as usize;
    match side {
        TrackpadSide::Left => LEFT_TRACKPAD_BUTTON_POSITIONS[index],
        TrackpadSide::Right => RIGHT_TRACKPAD_BUTTON_POSITIONS[index],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadMotionConfig {
    pub axis_transform: TrackpadAxisTransform,
    pub divisor: u16,
}

impl Default for TrackpadMotionConfig {
    fn default() -> Self {
        Self {
            axis_transform: TrackpadAxisTransform::new(false, false, false),
            divisor: DEFAULT_CURSOR_DIVISOR,
        }
    }
}

impl TrackpadMotionConfig {
    pub const fn new(axis_transform: TrackpadAxisTransform, divisor: u16) -> Self {
        Self {
            axis_transform,
            divisor,
        }
    }

    pub fn motion_event(
        self,
        side: TrackpadSide,
        relative_x: i16,
        relative_y: i16,
        remainder_x: &mut i32,
        remainder_y: &mut i32,
    ) -> Option<TrackpadMotionEvent> {
        if relative_x == 0 && relative_y == 0 {
            return None;
        }

        let (x, y) = self
            .axis_transform
            .apply((i32::from(relative_x), i32::from(relative_y)));
        let x = clamp_i32_to_i16(scale_with_remainder(x, self.divisor, remainder_x));
        let y = clamp_i32_to_i16(scale_with_remainder(y, self.divisor, remainder_y));

        if x == 0 && y == 0 {
            None
        } else {
            Some(TrackpadMotionEvent::cursor(side, x, y))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadScrollConfig {
    pub divisor: u16,
    pub low_speed_divisor: u16,
    pub low_speed_threshold: i16,
    pub inertia_divisor: u16,
    pub max_step: i16,
    pub inertia_max_step: i16,
}

impl Default for TrackpadScrollConfig {
    fn default() -> Self {
        Self {
            divisor: DEFAULT_SCROLL_DIVISOR,
            low_speed_divisor: DEFAULT_SCROLL_LOW_SPEED_DIVISOR,
            low_speed_threshold: DEFAULT_SCROLL_LOW_SPEED_THRESHOLD,
            inertia_divisor: DEFAULT_SCROLL_INERTIA_DIVISOR,
            max_step: DEFAULT_SCROLL_MAX_STEP,
            inertia_max_step: DEFAULT_SCROLL_INERTIA_MAX_STEP,
        }
    }
}

impl TrackpadScrollConfig {
    pub const fn new(divisor: u16) -> Self {
        Self {
            divisor,
            low_speed_divisor: DEFAULT_SCROLL_LOW_SPEED_DIVISOR,
            low_speed_threshold: DEFAULT_SCROLL_LOW_SPEED_THRESHOLD,
            inertia_divisor: DEFAULT_SCROLL_INERTIA_DIVISOR,
            max_step: DEFAULT_SCROLL_MAX_STEP,
            inertia_max_step: DEFAULT_SCROLL_INERTIA_MAX_STEP,
        }
    }

    pub fn scroll_event(
        self,
        side: TrackpadSide,
        relative_x: i16,
        relative_y: i16,
        remainder_x: &mut i32,
        remainder_y: &mut i32,
    ) -> Option<TrackpadMotionEvent> {
        self.scroll_event_with_limits(
            side,
            relative_x,
            relative_y,
            remainder_x,
            remainder_y,
            self.divisor,
            self.low_speed_divisor,
            self.low_speed_threshold,
            self.max_step,
        )
    }

    pub fn inertia_scroll_event(
        self,
        side: TrackpadSide,
        relative_x: i16,
        relative_y: i16,
        remainder_x: &mut i32,
        remainder_y: &mut i32,
    ) -> Option<TrackpadMotionEvent> {
        self.scroll_event_with_limits(
            side,
            relative_x,
            relative_y,
            remainder_x,
            remainder_y,
            self.inertia_divisor,
            self.inertia_divisor,
            0,
            self.inertia_max_step,
        )
    }

    fn scroll_event_with_limits(
        self,
        side: TrackpadSide,
        relative_x: i16,
        relative_y: i16,
        remainder_x: &mut i32,
        remainder_y: &mut i32,
        divisor: u16,
        low_speed_divisor: u16,
        low_speed_threshold: i16,
        max_step: i16,
    ) -> Option<TrackpadMotionEvent> {
        let divisor_x =
            effective_scroll_divisor(relative_x, divisor, low_speed_divisor, low_speed_threshold);
        let divisor_y =
            effective_scroll_divisor(relative_y, divisor, low_speed_divisor, low_speed_threshold);
        let pan = clamp_scroll_step(
            clamp_i32_to_i16(scale_scroll_axis_with_remainder(
                i32::from(relative_x),
                divisor_x,
                remainder_x,
                max_step,
            )),
            max_step,
        );
        let wheel = clamp_scroll_step(
            clamp_i32_to_i16(-scale_scroll_axis_with_remainder(
                i32::from(relative_y),
                divisor_y,
                remainder_y,
                max_step,
            )),
            max_step,
        );

        if pan == 0 && wheel == 0 {
            None
        } else {
            Some(TrackpadMotionEvent::scroll(side, wheel, pan))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadMotionEvent {
    pub side: TrackpadSide,
    pub x: i16,
    pub y: i16,
    pub wheel: i16,
    pub pan: i16,
    pub buttons: u8,
    pub button_state_valid: bool,
}

impl TrackpadMotionEvent {
    pub const fn cursor(side: TrackpadSide, x: i16, y: i16) -> Self {
        Self {
            side,
            x,
            y,
            wheel: 0,
            pan: 0,
            buttons: 0,
            button_state_valid: false,
        }
    }

    pub const fn scroll(side: TrackpadSide, wheel: i16, pan: i16) -> Self {
        Self {
            side,
            x: 0,
            y: 0,
            wheel,
            pan,
            buttons: 0,
            button_state_valid: false,
        }
    }

    pub const fn button_state(side: TrackpadSide, buttons: u8) -> Self {
        Self {
            side,
            x: 0,
            y: 0,
            wheel: 0,
            pan: 0,
            buttons,
            button_state_valid: true,
        }
    }

    pub const fn with_button_state(self, buttons: u8) -> Self {
        Self {
            buttons,
            button_state_valid: true,
            ..self
        }
    }

    pub fn into_rmk_event(self) -> Event {
        Event::Custom(self.encode())
    }

    pub fn encode(self) -> [u8; 16] {
        let mut payload = custom_event_payload(CUSTOM_EVENT_POINTER_MOTION);
        payload[5] = self.side.to_wire();
        payload[6..8].copy_from_slice(&self.x.to_le_bytes());
        payload[8..10].copy_from_slice(&self.y.to_le_bytes());
        payload[10..12].copy_from_slice(&self.wheel.to_le_bytes());
        payload[12..14].copy_from_slice(&self.pan.to_le_bytes());
        payload[14] = self.buttons;
        payload[15] = u8::from(self.button_state_valid);
        payload
    }

    pub fn decode(payload: [u8; 16]) -> Option<Self> {
        if !custom_event_is(payload, CUSTOM_EVENT_POINTER_MOTION) {
            return None;
        }

        let side = TrackpadSide::from_wire(payload[5])?;
        let x = i16::from_le_bytes([payload[6], payload[7]]);
        let y = i16::from_le_bytes([payload[8], payload[9]]);
        let wheel = i16::from_le_bytes([payload[10], payload[11]]);
        let pan = i16::from_le_bytes([payload[12], payload[13]]);
        let buttons = payload[14];
        let button_state_valid = payload[15] & 1 != 0;

        Some(Self {
            side,
            x,
            y,
            wheel,
            pan,
            buttons,
            button_state_valid,
        })
    }

    pub fn merge(self, next: Self) -> Self {
        if self.side != next.side {
            return next.capped();
        }

        Self {
            side: self.side,
            x: clamp_pending_motion(self.x.saturating_add(next.x)),
            y: clamp_pending_motion(self.y.saturating_add(next.y)),
            wheel: clamp_pending_scroll(self.wheel.saturating_add(next.wheel)),
            pan: clamp_pending_scroll(self.pan.saturating_add(next.pan)),
            buttons: if next.button_state_valid {
                next.buttons
            } else {
                self.buttons
            },
            button_state_valid: self.button_state_valid || next.button_state_valid,
        }
    }

    pub fn capped(self) -> Self {
        Self {
            side: self.side,
            x: clamp_pending_motion(self.x),
            y: clamp_pending_motion(self.y),
            wheel: clamp_pending_scroll(self.wheel),
            pan: clamp_pending_scroll(self.pan),
            buttons: self.buttons,
            button_state_valid: self.button_state_valid,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.x == 0 && self.y == 0 && self.wheel == 0 && self.pan == 0 && !self.button_state_valid
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadPinchReport {
    pub side: TrackpadSide,
    pub started: bool,
    pub ended: bool,
    pub motion: TrackpadMotionEvent,
}

impl TrackpadPinchReport {
    pub const fn new(
        side: TrackpadSide,
        started: bool,
        ended: bool,
        motion: TrackpadMotionEvent,
    ) -> Self {
        Self {
            side,
            started,
            ended,
            motion,
        }
    }

    pub fn encode(self) -> [u8; 16] {
        let mut payload = custom_event_payload(CUSTOM_EVENT_PINCH_REPORT);
        payload[5] = self.side.to_wire();
        payload[6] = u8::from(self.started) | (u8::from(self.ended) << 1);
        payload[8..10].copy_from_slice(&self.motion.wheel.to_le_bytes());
        payload[10..12].copy_from_slice(&self.motion.pan.to_le_bytes());
        payload
    }

    pub fn decode(payload: [u8; 16]) -> Option<Self> {
        if !custom_event_is(payload, CUSTOM_EVENT_PINCH_REPORT) {
            return None;
        }

        let side = TrackpadSide::from_wire(payload[5])?;
        let flags = payload[6];
        let wheel = i16::from_le_bytes([payload[8], payload[9]]);
        let pan = i16::from_le_bytes([payload[10], payload[11]]);

        Some(Self {
            side,
            started: flags & 1 != 0,
            ended: flags & 2 != 0,
            motion: TrackpadMotionEvent::scroll(side, wheel, pan),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadGestureEvent {
    Button {
        button: TrackpadButton,
        pressed: bool,
    },
    ReleaseAndClick(TrackpadButton),
    Click(TrackpadButton),
    Scroll(TrackpadScrollDelta),
    ScrollEnded,
    PinchStarted(i16),
    PinchWheel(i16),
    DeferredHoldPending,
}

impl TrackpadGestureEvent {
    pub fn button_events(self, side: TrackpadSide) -> TrackpadClickEvents {
        match self {
            Self::Button { .. } => TrackpadClickEvents::empty(side),
            Self::ReleaseAndClick(button) | Self::Click(button) => {
                TrackpadClickEvents::new(side, button)
            }
            Self::Scroll(_)
            | Self::ScrollEnded
            | Self::PinchStarted(_)
            | Self::PinchWheel(_)
            | Self::DeferredHoldPending => TrackpadClickEvents::empty(side),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadScrollDelta {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScrollMotionSample {
    x: i16,
    y: i16,
    ms: u32,
}

impl ScrollMotionSample {
    const EMPTY: Self = Self { x: 0, y: 0, ms: 0 };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScrollMotionHistory {
    samples: [ScrollMotionSample; SCROLL_INERTIA_HISTORY_SIZE],
    head: usize,
    count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InertiaConfig {
    interval_ms: u32,
    max_duration_ms: u32,
    decay_num: i32,
    decay_den: i32,
    fp_shift: u8,
    start_threshold: i32,
    min_velocity: i32,
    recent_window_ms: u32,
    stale_gap_ms: u32,
    min_samples: usize,
    min_avg_speed: i32,
}

const SCROLL_INERTIA_CONFIG: InertiaConfig = InertiaConfig {
    interval_ms: SCROLL_INERTIA_INTERVAL_MS,
    max_duration_ms: SCROLL_INERTIA_MAX_DURATION_MS,
    decay_num: SCROLL_INERTIA_DECAY_NUM,
    decay_den: SCROLL_INERTIA_DECAY_DEN,
    fp_shift: SCROLL_INERTIA_FP_SHIFT,
    start_threshold: SCROLL_INERTIA_START_THRESHOLD,
    min_velocity: SCROLL_INERTIA_MIN_VELOCITY,
    recent_window_ms: SCROLL_INERTIA_RECENT_WINDOW_MS,
    stale_gap_ms: SCROLL_INERTIA_STALE_GAP_MS,
    min_samples: SCROLL_INERTIA_MIN_SAMPLES,
    min_avg_speed: SCROLL_INERTIA_MIN_AVG_SPEED,
};

const CURSOR_INERTIA_CONFIG: InertiaConfig = InertiaConfig {
    interval_ms: CURSOR_INERTIA_INTERVAL_MS,
    max_duration_ms: CURSOR_INERTIA_MAX_DURATION_MS,
    decay_num: CURSOR_INERTIA_DECAY_NUM,
    decay_den: CURSOR_INERTIA_DECAY_DEN,
    fp_shift: SCROLL_INERTIA_FP_SHIFT,
    start_threshold: CURSOR_INERTIA_START_THRESHOLD,
    min_velocity: CURSOR_INERTIA_MIN_VELOCITY,
    recent_window_ms: CURSOR_INERTIA_RECENT_WINDOW_MS,
    stale_gap_ms: CURSOR_INERTIA_STALE_GAP_MS,
    min_samples: CURSOR_INERTIA_MIN_SAMPLES,
    min_avg_speed: CURSOR_INERTIA_MIN_AVG_SPEED,
};

impl ScrollMotionHistory {
    pub const fn new() -> Self {
        Self {
            samples: [ScrollMotionSample::EMPTY; SCROLL_INERTIA_HISTORY_SIZE],
            head: 0,
            count: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn push(&mut self, delta: TrackpadScrollDelta, now_ms: u32) {
        if delta.x == 0 && delta.y == 0 {
            return;
        }

        self.samples[self.head] = ScrollMotionSample {
            x: delta.x,
            y: delta.y,
            ms: now_ms,
        };
        self.head = (self.head + 1) % SCROLL_INERTIA_HISTORY_SIZE;
        if self.count < SCROLL_INERTIA_HISTORY_SIZE {
            self.count += 1;
        }
    }

    fn seed(self, now_ms: u32) -> Option<ScrollInertiaSeed> {
        self.seed_with(now_ms, SCROLL_INERTIA_CONFIG)
    }

    fn seed_with(self, now_ms: u32, config: InertiaConfig) -> Option<ScrollInertiaSeed> {
        let mut recent = [ScrollMotionSample::EMPTY; SCROLL_INERTIA_HISTORY_SIZE];
        let mut recent_count = 0usize;

        let mut i = 0usize;
        while i < self.count {
            let idx =
                (self.head + SCROLL_INERTIA_HISTORY_SIZE - 1 - i) % SCROLL_INERTIA_HISTORY_SIZE;
            let sample = self.samples[idx];
            if now_ms.wrapping_sub(sample.ms) > config.recent_window_ms {
                break;
            }
            recent[recent_count] = sample;
            recent_count += 1;
            i += 1;
        }

        if recent_count < config.min_samples {
            return None;
        }

        let latest = recent[0];
        if now_ms.wrapping_sub(latest.ms) > config.stale_gap_ms {
            return None;
        }

        let mut total_x = 0i32;
        let mut total_y = 0i32;
        let mut j = 0usize;
        while j < recent_count {
            total_x = total_x.saturating_add(i32::from(recent[j].x));
            total_y = total_y.saturating_add(i32::from(recent[j].y));
            j += 1;
        }

        if total_x == 0 && total_y == 0 {
            return None;
        }

        let dominant_total = if abs_i32(total_x) >= abs_i32(total_y) {
            total_x
        } else {
            total_y
        };
        let dominant_is_x = abs_i32(total_x) >= abs_i32(total_y);
        let mut consistent_count = 0usize;
        let mut k = 0usize;
        while k < recent_count {
            let sample = if dominant_is_x {
                i32::from(recent[k].x)
            } else {
                i32::from(recent[k].y)
            };
            if (sample > 0 && dominant_total > 0) || (sample < 0 && dominant_total < 0) {
                consistent_count += 1;
            }
            k += 1;
        }

        if consistent_count < config.min_samples {
            return None;
        }

        let earliest = recent[recent_count - 1];
        let mut span_ms = latest.ms.wrapping_sub(earliest.ms);
        if span_ms < config.interval_ms {
            span_ms = config.interval_ms;
        }

        let avg_speed = ((abs_i32(total_x) + abs_i32(total_y))
            .saturating_mul(config.interval_ms as i32))
            / span_ms as i32;
        if avg_speed < config.min_avg_speed {
            return None;
        }

        let interval = i64::from(config.interval_ms);
        let span = i64::from(span_ms);
        let shift = u32::from(config.fp_shift);
        Some(ScrollInertiaSeed {
            vx_fp: clamp_i64_to_i32(((i64::from(total_x) * interval) << shift) / span),
            vy_fp: clamp_i64_to_i32(((i64::from(total_y) * interval) << shift) / span),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScrollInertiaSeed {
    vx_fp: i32,
    vy_fp: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScrollInertiaState {
    active: bool,
    vx_fp: i32,
    vy_fp: i32,
    accum_x_fp: i32,
    accum_y_fp: i32,
    elapsed_ms: u32,
    last_ms: u32,
}

impl ScrollInertiaState {
    pub const fn new() -> Self {
        Self {
            active: false,
            vx_fp: 0,
            vy_fp: 0,
            accum_x_fp: 0,
            accum_y_fp: 0,
            elapsed_ms: 0,
            last_ms: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn start(&mut self, seed: ScrollInertiaSeed, now_ms: u32) {
        self.start_with(seed, now_ms, SCROLL_INERTIA_CONFIG);
    }

    fn start_with(&mut self, seed: ScrollInertiaSeed, now_ms: u32, config: InertiaConfig) {
        let start_threshold_fp = config.start_threshold << config.fp_shift;
        if abs_i32(seed.vx_fp) < start_threshold_fp && abs_i32(seed.vy_fp) < start_threshold_fp {
            self.reset();
            return;
        }

        self.active = true;
        self.vx_fp = seed.vx_fp;
        self.vy_fp = seed.vy_fp;
        self.accum_x_fp = 0;
        self.accum_y_fp = 0;
        self.elapsed_ms = 0;
        self.last_ms = now_ms;
    }

    fn step(&mut self, now_ms: u32) -> Option<TrackpadScrollDelta> {
        self.step_with(now_ms, SCROLL_INERTIA_CONFIG)
    }

    fn step_with(&mut self, now_ms: u32, config: InertiaConfig) -> Option<TrackpadScrollDelta> {
        if !self.active {
            return None;
        }

        let dt_ms = now_ms.wrapping_sub(self.last_ms);
        if dt_ms < config.interval_ms {
            return None;
        }

        self.accum_x_fp = self.accum_x_fp.saturating_add(self.vx_fp);
        self.accum_y_fp = self.accum_y_fp.saturating_add(self.vy_fp);
        self.vx_fp = (self.vx_fp.saturating_mul(config.decay_num)) / config.decay_den;
        self.vy_fp = (self.vy_fp.saturating_mul(config.decay_num)) / config.decay_den;
        self.last_ms = now_ms;
        self.elapsed_ms = self.elapsed_ms.saturating_add(config.interval_ms);

        let out_x = self.accum_x_fp >> config.fp_shift;
        let out_y = self.accum_y_fp >> config.fp_shift;
        self.accum_x_fp -= out_x << config.fp_shift;
        self.accum_y_fp -= out_y << config.fp_shift;

        let min_v_fp = config.min_velocity << config.fp_shift;
        if self.elapsed_ms >= config.max_duration_ms
            || (abs_i32(self.vx_fp) < min_v_fp && abs_i32(self.vy_fp) < min_v_fp)
        {
            self.active = false;
        }

        if out_x == 0 && out_y == 0 {
            None
        } else {
            Some(TrackpadScrollDelta {
                x: clamp_i32_to_i16(out_x),
                y: clamp_i32_to_i16(out_y),
            })
        }
    }
}

pub struct TrackpadClickEvents {
    side: TrackpadSide,
    button: TrackpadButton,
    next_pressed: u8,
}

impl TrackpadClickEvents {
    pub const fn new(side: TrackpadSide, button: TrackpadButton) -> Self {
        Self {
            side,
            button,
            next_pressed: 0,
        }
    }

    pub const fn empty(side: TrackpadSide) -> Self {
        Self {
            side,
            button: TrackpadButton::LeftClick,
            next_pressed: 2,
        }
    }
}

impl Iterator for TrackpadClickEvents {
    type Item = TrackpadButtonEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let pressed = match self.next_pressed {
            0 => true,
            1 => false,
            _ => return None,
        };
        self.next_pressed += 1;

        Some(TrackpadButtonEvent {
            button: self.button,
            position: trackpad_button_position(self.side, self.button),
            pressed,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadGestureConfig {
    pub one_finger_tap_max_ms: u32,
    pub one_finger_drag_hold_ms: u32,
    pub one_finger_tap_move: u16,
    pub two_finger_tap_max_ms: u32,
    pub two_finger_tap_move: u16,
    pub three_finger_tap_max_ms: u32,
    pub three_finger_tap_move: u16,
    pub three_finger_swipe_move: u16,
    pub axis_transform: TrackpadAxisTransform,
}

impl Default for TrackpadGestureConfig {
    fn default() -> Self {
        Self {
            one_finger_tap_max_ms: 250,
            one_finger_drag_hold_ms: DEFAULT_ONE_FINGER_DRAG_HOLD_MS,
            one_finger_tap_move: 50,
            two_finger_tap_max_ms: 250,
            two_finger_tap_move: 50,
            three_finger_tap_max_ms: 200,
            three_finger_tap_move: 35,
            three_finger_swipe_move: 200,
            axis_transform: TrackpadAxisTransform::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrackpadAxisTransform {
    pub invert_x: bool,
    pub invert_y: bool,
    pub swap_xy: bool,
}

impl TrackpadAxisTransform {
    pub const fn new(invert_x: bool, invert_y: bool, swap_xy: bool) -> Self {
        Self {
            invert_x,
            invert_y,
            swap_xy,
        }
    }

    fn apply(self, position: (i32, i32)) -> (i32, i32) {
        let (mut x, mut y) = position;

        if self.swap_xy {
            core::mem::swap(&mut x, &mut y);
        }
        if self.invert_x {
            x = -x;
        }
        if self.invert_y {
            y = -y;
        }

        (x, y)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadGestureRecognizer {
    config: TrackpadGestureConfig,
    one_finger: OneFingerState,
    two_finger: TwoFingerState,
    three_finger: ThreeFingerState,
    prev_frame: CoordinateFrame,
    finger_history: FingerHistory,
    two_finger_one_lead_valid: bool,
    three_finger_one_lead_valid: bool,
    three_finger_two_lead_valid: bool,
    suppress_cursor_tail: bool,
    hold_button: Option<TrackpadButton>,
    one_finger_click_pending: bool,
    one_finger_click_pending_ms: u32,
    two_finger_click_pending: bool,
    two_finger_click_pending_ms: u32,
    three_finger_click_pending: bool,
    three_finger_click_pending_ms: u32,
}

impl TrackpadGestureRecognizer {
    pub const fn new(config: TrackpadGestureConfig) -> Self {
        Self {
            config,
            one_finger: OneFingerState::new(),
            two_finger: TwoFingerState::new(),
            three_finger: ThreeFingerState::new(),
            prev_frame: CoordinateFrame::empty(),
            finger_history: FingerHistory::new(),
            two_finger_one_lead_valid: false,
            three_finger_one_lead_valid: false,
            three_finger_two_lead_valid: false,
            suppress_cursor_tail: false,
            hold_button: None,
            one_finger_click_pending: false,
            one_finger_click_pending_ms: 0,
            two_finger_click_pending: false,
            two_finger_click_pending_ms: 0,
            three_finger_click_pending: false,
            three_finger_click_pending_ms: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(TrackpadGestureConfig::default())
    }

    pub const fn config(&self) -> TrackpadGestureConfig {
        self.config
    }

    pub fn reset(&mut self) {
        self.one_finger.reset();
        self.two_finger.reset();
        self.three_finger.reset();
        self.prev_frame = CoordinateFrame::empty();
        self.finger_history.reset();
        self.two_finger_one_lead_valid = false;
        self.three_finger_one_lead_valid = false;
        self.three_finger_two_lead_valid = false;
        self.suppress_cursor_tail = false;
        self.hold_button = None;
        self.clear_one_finger_click_pending();
        self.clear_two_finger_click_pending();
        self.clear_three_finger_click_pending();
    }

    pub const fn cursor_suppressed(self) -> bool {
        self.suppress_cursor_tail
    }

    fn clear_one_finger_click_pending(&mut self) {
        self.one_finger_click_pending = false;
        self.one_finger_click_pending_ms = 0;
    }

    fn clear_two_finger_click_pending(&mut self) {
        self.two_finger_click_pending = false;
        self.two_finger_click_pending_ms = 0;
    }

    fn clear_three_finger_click_pending(&mut self) {
        self.three_finger_click_pending = false;
        self.three_finger_click_pending_ms = 0;
    }

    fn arm_deferred_hold(&mut self, button: TrackpadButton, now_ms: u32) {
        match button {
            TrackpadButton::LeftClick => {
                self.one_finger_click_pending = true;
                self.one_finger_click_pending_ms = now_ms;
            }
            TrackpadButton::RightClick => {
                self.two_finger_click_pending = true;
                self.two_finger_click_pending_ms = now_ms;
            }
            TrackpadButton::MiddleClick => {
                self.three_finger_click_pending = true;
                self.three_finger_click_pending_ms = now_ms;
            }
            TrackpadButton::GestureLeft
            | TrackpadButton::GestureRight
            | TrackpadButton::GestureUp
            | TrackpadButton::GestureDown
            | TrackpadButton::Pinch => {}
        }
    }

    fn emit_hold_press(
        &mut self,
        button: TrackpadButton,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        if let Some(held) = self.hold_button {
            self.hold_button = None;
            return Some(TrackpadGestureEvent::Button {
                button: held,
                pressed: false,
            });
        }

        self.hold_button = Some(button);
        self.arm_deferred_hold(button, now_ms);
        Some(TrackpadGestureEvent::Button {
            button,
            pressed: true,
        })
    }

    fn release_hold(&mut self, button: TrackpadButton) -> Option<TrackpadGestureEvent> {
        if self.hold_button != Some(button) {
            return None;
        }

        self.hold_button = None;
        Some(TrackpadGestureEvent::Button {
            button,
            pressed: false,
        })
    }

    fn clear_pending_for_button(&mut self, button: TrackpadButton) {
        match button {
            TrackpadButton::LeftClick => self.clear_one_finger_click_pending(),
            TrackpadButton::RightClick => self.clear_two_finger_click_pending(),
            TrackpadButton::MiddleClick => self.clear_three_finger_click_pending(),
            TrackpadButton::GestureLeft
            | TrackpadButton::GestureRight
            | TrackpadButton::GestureUp
            | TrackpadButton::GestureDown
            | TrackpadButton::Pinch => {}
        }
    }

    fn release_deferred_hold(&mut self, button: TrackpadButton) -> Option<TrackpadGestureEvent> {
        self.clear_pending_for_button(button);
        self.release_hold(button)
    }

    fn release_hold_and_click(&mut self, button: TrackpadButton) -> TrackpadGestureEvent {
        if self.hold_button == Some(button) {
            self.hold_button = None;
            TrackpadGestureEvent::ReleaseAndClick(button)
        } else {
            TrackpadGestureEvent::Click(button)
        }
    }

    fn tap_start_allowed(&self, prev_frame: CoordinateFrame, now_ms: u32) -> bool {
        prev_frame.finger_count() == 0
            || self
                .finger_history
                .has_recent(0, now_ms, TAP_REENTRY_WINDOW_MS)
    }

    fn release_pending_deferred_holds(
        &mut self,
        finger_count: u8,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        if self.one_finger_click_pending {
            let elapsed_ms = now_ms.wrapping_sub(self.one_finger_click_pending_ms);
            if deferred_hold_expired(finger_count, 1, elapsed_ms, ONE_FINGER_TAPDRAG_GAP_MAX_MS) {
                if let Some(event) = self.release_deferred_hold(TrackpadButton::LeftClick) {
                    return Some(event);
                }
            }
        }

        if self.two_finger_click_pending {
            let elapsed_ms = now_ms.wrapping_sub(self.two_finger_click_pending_ms);
            if deferred_hold_expired(finger_count, 2, elapsed_ms, TWO_FINGER_TAPDRAG_GAP_MAX_MS) {
                if let Some(event) = self.release_deferred_hold(TrackpadButton::RightClick) {
                    return Some(event);
                }
            }
        }

        if self.three_finger_click_pending {
            let elapsed_ms = now_ms.wrapping_sub(self.three_finger_click_pending_ms);
            if finger_count == 3 && elapsed_ms > THREE_FINGER_TAPDRAG_GAP_MAX_MS {
                if let Some(event) = self.release_deferred_hold(TrackpadButton::MiddleClick) {
                    return Some(event);
                }
            } else if finger_count != 0 && finger_count != 3 {
                if elapsed_ms < THREE_FINGER_TAPDRAG_GAP_MAX_MS {
                    return Some(TrackpadGestureEvent::DeferredHoldPending);
                }
                if let Some(event) = self.release_deferred_hold(TrackpadButton::MiddleClick) {
                    return Some(event);
                }
            } else if finger_count == 0 && elapsed_ms >= THREE_FINGER_TAPDRAG_GAP_MAX_MS {
                if let Some(event) = self.release_deferred_hold(TrackpadButton::MiddleClick) {
                    return Some(event);
                }
            }
        }

        None
    }

    pub fn update(&mut self, frame: CoordinateFrame, now_ms: u32) -> Option<TrackpadGestureEvent> {
        let finger_count = frame.finger_count();
        let prev_frame = self.prev_frame;
        let event = self.process_frame(frame, prev_frame, now_ms);

        self.update_prev_frame(frame, prev_frame);
        self.finger_history.push(finger_count, now_ms);

        event
    }

    fn process_frame(
        &mut self,
        frame: CoordinateFrame,
        prev_frame: CoordinateFrame,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        let finger_count = frame.finger_count();
        let prev_finger_count = prev_frame.finger_count();
        let mut event = None;

        if prev_finger_count == 2 && finger_count == 1 {
            self.suppress_cursor_tail = true;
        }
        if finger_count == 0 || finger_count >= 2 {
            self.suppress_cursor_tail = false;
        }

        if let Some(event) = self.release_pending_deferred_holds(finger_count, now_ms) {
            return Some(event);
        }

        if finger_count == 3 && self.one_finger.active {
            self.three_finger_one_lead_valid = self.one_finger.tap_lead_valid(
                now_ms,
                THREE_FINGER_ONE_LEAD_MAX_MS,
                self.config.one_finger_tap_move,
            );
            if self.one_finger.hold_sent {
                self.one_finger.reset();
                self.suppress_cursor_tail = true;
                return self.release_hold(TrackpadButton::LeftClick);
            }
            self.one_finger.reset();
        } else {
            self.three_finger_one_lead_valid = false;
        }

        if finger_count == 3 && self.two_finger.active {
            self.three_finger_two_lead_valid = self.two_finger.tap_lead_valid(
                now_ms,
                THREE_FINGER_TWO_LEAD_MAX_MS,
                self.config.two_finger_tap_move,
            );
            if self.two_finger.mode == TwoFingerMode::Scroll {
                event = Some(TrackpadGestureEvent::ScrollEnded);
            } else if self.two_finger.mode == TwoFingerMode::Pinch {
                event = Some(TrackpadGestureEvent::Button {
                    button: TrackpadButton::Pinch,
                    pressed: false,
                });
            }
            if self.two_finger.hold_sent {
                self.two_finger.reset();
                self.two_finger_one_lead_valid = false;
                self.suppress_cursor_tail = true;
                return self.release_hold(TrackpadButton::RightClick);
            }
            self.two_finger.reset();
            self.two_finger_one_lead_valid = false;
            if event.is_some() {
                return event;
            }
        } else {
            self.three_finger_two_lead_valid = false;
        }

        if finger_count == 2 && self.one_finger.active {
            self.two_finger_one_lead_valid = self.one_finger.tap_lead_valid(
                now_ms,
                TWO_FINGER_ONE_LEAD_MAX_MS,
                self.config.one_finger_tap_move,
            );
            if self.one_finger.hold_sent {
                self.one_finger.reset();
                self.suppress_cursor_tail = true;
                return self.release_hold(TrackpadButton::LeftClick);
            }
            self.one_finger.reset();
        } else if finger_count != 2 {
            self.two_finger_one_lead_valid = false;
        }

        if finger_count != 1 && self.one_finger.active {
            event = self.update_one_finger(frame, prev_frame, now_ms);
        }
        if finger_count != 2 && self.two_finger.active && event.is_none() {
            event = self.update_two_finger(frame, prev_frame, now_ms);
        }
        if finger_count != 3 && self.three_finger.active && event.is_none() {
            event = self.update_three_finger(frame, prev_frame, now_ms);
        }

        if event.is_some() {
            return event;
        }

        match finger_count {
            1 if self.two_finger.release_pending => None,
            1 if self.three_finger.release_pending => None,
            1 => self.update_one_finger(frame, prev_frame, now_ms),
            2 if self.three_finger.release_pending => None,
            2 => self.update_two_finger(frame, prev_frame, now_ms),
            3 => self.update_three_finger(frame, prev_frame, now_ms),
            _ => None,
        }
    }

    fn update_one_finger(
        &mut self,
        frame: CoordinateFrame,
        prev_frame: CoordinateFrame,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        let one_now = frame.finger_count() == 1;
        let have_xy = one_now
            .then(|| get_finger1_position(frame, prev_frame, self.config.axis_transform))
            .flatten();

        if !self.one_finger.active && one_now {
            let Some((x, y)) = have_xy else {
                return None;
            };

            let tapdrag_second_touch = take_pending_tapdrag_second_touch(
                &mut self.one_finger_click_pending,
                &mut self.one_finger_click_pending_ms,
                now_ms,
                ONE_FINGER_TAPDRAG_GAP_MAX_MS,
            );

            self.one_finger.start(
                now_ms,
                x,
                y,
                !tapdrag_second_touch && self.tap_start_allowed(prev_frame, now_ms),
                tapdrag_second_touch,
            );
            return None;
        }

        if !self.one_finger.active {
            return None;
        }

        if one_now {
            if let Some((x, y)) = have_xy {
                self.one_finger.update_position(x, y);
            }
            self.one_finger.cancel_tap_if_needed(
                now_ms,
                self.config.one_finger_tap_max_ms,
                self.config.one_finger_tap_move,
            );
            self.one_finger.cancel_hold_if_needed(
                now_ms,
                self.config.one_finger_tap_max_ms,
                self.config.one_finger_tap_move,
            );
            return None;
        }

        if self.one_finger.tapdrag_second_touch {
            let second_tap_detected = frame.finger_count() == 0
                && self.one_finger.hold_valid(
                    now_ms,
                    self.config.one_finger_tap_max_ms,
                    self.config.one_finger_tap_move,
                );
            let hold_sent = self.one_finger.hold_sent;
            self.one_finger.reset();
            self.suppress_cursor_tail = true;
            return if hold_sent {
                if second_tap_detected {
                    Some(self.release_hold_and_click(TrackpadButton::LeftClick))
                } else {
                    self.release_hold(TrackpadButton::LeftClick)
                }
            } else {
                None
            };
        }

        let tap_detected = frame.finger_count() == 0
            && self.one_finger.tap_valid(
                now_ms,
                self.config.one_finger_tap_max_ms,
                self.config.one_finger_tap_move,
            );
        self.one_finger.reset();
        if tap_detected {
            self.emit_hold_press(TrackpadButton::LeftClick, now_ms)
        } else {
            None
        }
    }

    fn update_two_finger(
        &mut self,
        frame: CoordinateFrame,
        prev_frame: CoordinateFrame,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        let two_now = frame.finger_count() == 2;
        let current_positions_confident = two_finger_positions_confident(frame);
        let have_xy = two_now
            .then(|| get_two_finger_metrics(frame, prev_frame, self.config.axis_transform))
            .flatten();

        if !self.two_finger.active && two_now {
            let tapdrag_reentry = pending_tapdrag_second_touch(
                self.two_finger_click_pending,
                self.two_finger_click_pending_ms,
                now_ms,
                TWO_FINGER_TAPDRAG_GAP_MAX_MS,
            );
            let Some(metrics) = have_xy else {
                if !tapdrag_reentry {
                    if let Some(delta) = two_finger_relative_scroll_delta(
                        frame,
                        self.config.axis_transform,
                        TWO_FINGER_RELATIVE_SCROLL_START_MOVE,
                    ) {
                        self.two_finger.start_relative_scroll(now_ms);
                        return Some(TrackpadGestureEvent::Scroll(delta));
                    }
                }
                return None;
            };

            let tapdrag_second_touch = take_pending_tapdrag_second_touch(
                &mut self.two_finger_click_pending,
                &mut self.two_finger_click_pending_ms,
                now_ms,
                TWO_FINGER_TAPDRAG_GAP_MAX_MS,
            );
            let relative_scroll = if tapdrag_second_touch || current_positions_confident {
                None
            } else {
                two_finger_relative_scroll_delta(
                    frame,
                    self.config.axis_transform,
                    TWO_FINGER_RELATIVE_SCROLL_START_MOVE,
                )
            };

            self.two_finger.start(
                now_ms,
                metrics,
                !tapdrag_second_touch
                    && (self.tap_start_allowed(prev_frame, now_ms)
                        || self.two_finger_one_lead_valid),
                tapdrag_second_touch,
            );
            self.two_finger_one_lead_valid = false;
            if let Some(delta) = relative_scroll {
                if !current_positions_confident {
                    self.two_finger.mark_metrics_stale();
                }
                self.two_finger.mode = TwoFingerMode::Scroll;
                self.two_finger.tap_candidate = false;
                self.two_finger.hold_candidate = false;
                return Some(TrackpadGestureEvent::Scroll(delta));
            }
            return None;
        }

        if !self.two_finger.active {
            self.two_finger_one_lead_valid = false;
            return None;
        }

        self.two_finger_one_lead_valid = false;

        if two_now {
            let mut step = TwoFingerStep::default();
            if let Some(metrics) = have_xy {
                if self.two_finger.metrics_stale {
                    self.two_finger.sync_metrics(metrics);
                    return None;
                }
                step = self.two_finger.update_metrics(metrics);
            }
            self.two_finger.cancel_tap_if_needed(
                now_ms,
                self.config.two_finger_tap_max_ms,
                self.config.two_finger_tap_move,
            );
            self.two_finger.cancel_hold_if_needed(
                now_ms,
                self.config.two_finger_tap_max_ms,
                self.config.two_finger_tap_move,
            );
            if self.two_finger.tapdrag_second_touch {
                return None;
            }
            if !current_positions_confident && self.two_finger.mode != TwoFingerMode::Pinch {
                let min_move = if self.two_finger.mode == TwoFingerMode::Scroll {
                    0
                } else {
                    TWO_FINGER_RELATIVE_SCROLL_START_MOVE
                };
                if let Some(delta) =
                    two_finger_relative_scroll_delta(frame, self.config.axis_transform, min_move)
                {
                    self.two_finger.mark_metrics_stale();
                    self.two_finger.mode = TwoFingerMode::Scroll;
                    self.two_finger.tap_candidate = false;
                    self.two_finger.hold_candidate = false;
                    self.two_finger.release_pending = false;
                    return Some(TrackpadGestureEvent::Scroll(delta));
                }
            }
            self.two_finger.classify_mode();
            self.two_finger.release_pending = false;
            if self.two_finger.mode == TwoFingerMode::Scroll {
                return Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                    x: clamp_i32_to_i16(step.x),
                    y: clamp_i32_to_i16(step.y),
                }));
            }
            if self.two_finger.mode == TwoFingerMode::Pinch {
                if !self.two_finger.pinch_button_sent {
                    self.two_finger.pinch_button_sent = true;
                    return Some(TrackpadGestureEvent::PinchStarted(
                        self.two_finger.pinch_wheel(step.distance),
                    ));
                }
                let wheel = self.two_finger.pinch_wheel(step.distance);
                if wheel != 0 {
                    return Some(TrackpadGestureEvent::PinchWheel(wheel));
                }
            }
            return None;
        }

        if self.two_finger.release_pending {
            let pending_ms = now_ms.wrapping_sub(self.two_finger.release_pending_ms);
            if frame.finger_count() == 1 && pending_ms <= TWO_FINGER_RELEASE_PENDING_MAX_MS {
                self.suppress_cursor_tail = true;
                return None;
            }

            let tap_detected = frame.finger_count() == 0
                && pending_ms <= TWO_FINGER_RELEASE_PENDING_MAX_MS
                && self.two_finger.tap_valid(
                    now_ms,
                    self.config.two_finger_tap_max_ms,
                    self.config.two_finger_tap_move,
                );
            self.two_finger.reset();
            return if tap_detected {
                self.emit_hold_press(TrackpadButton::RightClick, now_ms)
            } else {
                None
            };
        }

        if self.two_finger.mode == TwoFingerMode::Scroll {
            self.suppress_cursor_tail = true;
            self.two_finger.reset();
            return Some(TrackpadGestureEvent::ScrollEnded);
        }
        if self.two_finger.mode == TwoFingerMode::Pinch {
            self.suppress_cursor_tail = true;
            self.two_finger.reset();
            return Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::Pinch,
                pressed: false,
            });
        }

        if self.two_finger.tapdrag_second_touch {
            let second_tap_detected = frame.finger_count() == 0
                && self.two_finger.hold_valid(
                    now_ms,
                    self.config.two_finger_tap_max_ms,
                    self.config.two_finger_tap_move,
                );
            if frame.finger_count() > 0 {
                self.two_finger.hold_candidate = false;
                return None;
            }

            let hold_sent = self.two_finger.hold_sent;
            self.two_finger.reset();
            self.suppress_cursor_tail = true;
            return if hold_sent {
                if second_tap_detected {
                    Some(self.release_hold_and_click(TrackpadButton::RightClick))
                } else {
                    self.release_hold(TrackpadButton::RightClick)
                }
            } else {
                None
            };
        }

        if self.two_finger.tap_valid(
            now_ms,
            self.config.two_finger_tap_max_ms,
            self.config.two_finger_tap_move,
        ) {
            match frame.finger_count() {
                1 => {
                    self.two_finger.release_pending = true;
                    self.two_finger.release_pending_ms = now_ms;
                    self.suppress_cursor_tail = true;
                    return None;
                }
                0 => {
                    self.two_finger.reset();
                    return self.emit_hold_press(TrackpadButton::RightClick, now_ms);
                }
                _ => {}
            }
        }

        self.two_finger.reset();
        None
    }

    fn update_three_finger(
        &mut self,
        frame: CoordinateFrame,
        prev_frame: CoordinateFrame,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        let three_now = frame.finger_count() == 3;
        let have_xy = three_now
            .then(|| get_finger1_position(frame, prev_frame, self.config.axis_transform))
            .flatten();

        if !self.three_finger.active && three_now {
            let Some((x, y)) = have_xy else {
                return None;
            };

            let tapdrag_second_touch = take_pending_tapdrag_second_touch(
                &mut self.three_finger_click_pending,
                &mut self.three_finger_click_pending_ms,
                now_ms,
                THREE_FINGER_TAPDRAG_GAP_MAX_MS,
            );

            self.three_finger.start(
                now_ms,
                x,
                y,
                !tapdrag_second_touch
                    && (self.tap_start_allowed(prev_frame, now_ms)
                        || self.three_finger_one_lead_valid
                        || self.three_finger_two_lead_valid),
                tapdrag_second_touch,
            );
            self.three_finger_one_lead_valid = false;
            self.three_finger_two_lead_valid = false;
            return None;
        }

        if !self.three_finger.active {
            self.three_finger_one_lead_valid = false;
            self.three_finger_two_lead_valid = false;
            return None;
        }

        self.three_finger_one_lead_valid = false;
        self.three_finger_two_lead_valid = false;

        if three_now {
            if let Some((x, y)) = have_xy {
                self.three_finger.update_position(x, y);
            }
            self.three_finger.cancel_tap_if_needed(
                now_ms,
                self.config.three_finger_tap_max_ms,
                self.config.three_finger_tap_move,
            );
            self.three_finger.cancel_hold_if_needed(
                now_ms,
                self.config.three_finger_tap_max_ms,
                self.config.three_finger_tap_move,
            );
            if self.three_finger.tapdrag_second_touch {
                return None;
            }
            self.three_finger.release_pending = false;
            if !self.three_finger.swipe_sent && !self.three_finger.hold_sent {
                if let Some(button) = self
                    .three_finger
                    .swipe_button(self.config.three_finger_swipe_move)
                {
                    self.three_finger.swipe_sent = true;
                    self.three_finger.tap_candidate = false;
                    return Some(TrackpadGestureEvent::Click(button));
                }
            }
            return None;
        }

        if self.three_finger.tapdrag_second_touch {
            let second_tap_detected = frame.finger_count() == 0
                && self.three_finger.hold_valid(
                    now_ms,
                    self.config.three_finger_tap_max_ms,
                    self.config.three_finger_tap_move,
                );
            if frame.finger_count() > 0 {
                self.three_finger.hold_candidate = false;
                return None;
            }

            let hold_sent = self.three_finger.hold_sent;
            self.three_finger.reset();
            self.suppress_cursor_tail = true;
            return if hold_sent {
                if second_tap_detected {
                    Some(self.release_hold_and_click(TrackpadButton::MiddleClick))
                } else {
                    self.release_hold(TrackpadButton::MiddleClick)
                }
            } else {
                None
            };
        }

        if self.three_finger.release_pending {
            let pending_ms = now_ms.wrapping_sub(self.three_finger.release_pending_ms);
            if frame.finger_count() > 0
                && frame.finger_count() < 3
                && pending_ms <= THREE_FINGER_RELEASE_PENDING_MAX_MS
            {
                self.suppress_cursor_tail = true;
                return None;
            }

            let tap_detected = frame.finger_count() == 0
                && pending_ms <= THREE_FINGER_RELEASE_PENDING_MAX_MS
                && !self.three_finger.hold_sent
                && !self.three_finger.swipe_sent
                && self.three_finger.tap_valid(
                    now_ms,
                    self.config.three_finger_tap_max_ms,
                    self.config.three_finger_tap_move,
                );
            self.three_finger.reset();
            return if tap_detected {
                self.emit_hold_press(TrackpadButton::MiddleClick, now_ms)
            } else {
                None
            };
        }

        if !self.three_finger.hold_sent
            && !self.three_finger.swipe_sent
            && self.three_finger.tap_valid(
                now_ms,
                self.config.three_finger_tap_max_ms,
                self.config.three_finger_tap_move,
            )
        {
            if frame.finger_count() > 0 && frame.finger_count() < 3 {
                self.three_finger.release_pending = true;
                self.three_finger.release_pending_ms = now_ms;
                self.suppress_cursor_tail = true;
                return None;
            }

            if frame.finger_count() == 0 {
                self.three_finger.reset();
                return self.emit_hold_press(TrackpadButton::MiddleClick, now_ms);
            }
        }

        self.three_finger.reset();
        None
    }

    fn update_prev_frame(&mut self, frame: CoordinateFrame, prev_frame: CoordinateFrame) {
        let mut next = frame;

        if frame.finger_count() == 0 {
            next.finger1 = FingerPosition::default();
            next.finger2 = FingerPosition::default();
            self.prev_frame = next;
            return;
        }

        if !finger1_valid(frame) {
            next.finger1 = prev_frame.finger1;
        }

        if frame.finger_count() < 2 {
            next.finger2 = FingerPosition::default();
            self.prev_frame = next;
            return;
        }

        if !finger2_valid(frame) {
            next.finger2 = prev_frame.finger2;
        }

        self.prev_frame = next;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OneFingerState {
    active: bool,
    hold_sent: bool,
    tap_candidate: bool,
    hold_candidate: bool,
    tapdrag_second_touch: bool,
    down_ms: u32,
    dx: i32,
    dy: i32,
    last_x: i32,
    last_y: i32,
}

impl OneFingerState {
    pub const fn new() -> Self {
        Self {
            active: false,
            hold_sent: false,
            tap_candidate: false,
            hold_candidate: false,
            tapdrag_second_touch: false,
            down_ms: 0,
            dx: 0,
            dy: 0,
            last_x: 0,
            last_y: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn start(
        &mut self,
        now_ms: u32,
        x: i32,
        y: i32,
        tap_candidate: bool,
        tapdrag_second_touch: bool,
    ) {
        self.active = true;
        self.hold_sent = tapdrag_second_touch;
        self.tap_candidate = tap_candidate;
        self.hold_candidate = tapdrag_second_touch;
        self.tapdrag_second_touch = tapdrag_second_touch;
        self.down_ms = now_ms;
        self.dx = 0;
        self.dy = 0;
        self.last_x = x;
        self.last_y = y;
    }

    fn update_position(&mut self, x: i32, y: i32) {
        self.dx = self.dx.saturating_add(x.saturating_sub(self.last_x));
        self.dy = self.dy.saturating_add(y.saturating_sub(self.last_y));
        self.last_x = x;
        self.last_y = y;
    }

    fn cancel_tap_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.tap_valid(now_ms, max_ms, move_threshold) {
            self.tap_candidate = false;
        }
    }

    fn cancel_hold_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.hold_valid(now_ms, max_ms, move_threshold) {
            self.hold_candidate = false;
        }
    }

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.dx) <= i32::from(move_threshold)
            && abs_i32(self.dy) <= i32::from(move_threshold)
    }

    fn hold_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.hold_candidate
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.dx) <= i32::from(move_threshold)
            && abs_i32(self.dy) <= i32::from(move_threshold)
    }

    fn tap_lead_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_valid(now_ms, max_ms, move_threshold)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TwoFingerMode {
    #[default]
    None,
    Scroll,
    Pinch,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TwoFingerStep {
    x: i32,
    y: i32,
    distance: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TwoFingerState {
    active: bool,
    hold_sent: bool,
    tap_candidate: bool,
    hold_candidate: bool,
    tapdrag_second_touch: bool,
    release_pending: bool,
    down_ms: u32,
    release_pending_ms: u32,
    centroid_dx: i32,
    centroid_dy: i32,
    distance_delta: i32,
    centroid_last_x: i32,
    centroid_last_y: i32,
    distance_last: i32,
    metrics_stale: bool,
    mode: TwoFingerMode,
    pinch_wheel_remainder: i32,
    pinch_button_sent: bool,
}

impl TwoFingerState {
    pub const fn new() -> Self {
        Self {
            active: false,
            hold_sent: false,
            tap_candidate: false,
            hold_candidate: false,
            tapdrag_second_touch: false,
            release_pending: false,
            down_ms: 0,
            release_pending_ms: 0,
            centroid_dx: 0,
            centroid_dy: 0,
            distance_delta: 0,
            centroid_last_x: 0,
            centroid_last_y: 0,
            distance_last: 0,
            metrics_stale: false,
            mode: TwoFingerMode::None,
            pinch_wheel_remainder: 0,
            pinch_button_sent: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn start(
        &mut self,
        now_ms: u32,
        metrics: TwoFingerMetrics,
        tap_candidate: bool,
        tapdrag_second_touch: bool,
    ) {
        self.active = true;
        self.hold_sent = tapdrag_second_touch;
        self.tap_candidate = tap_candidate;
        self.hold_candidate = tapdrag_second_touch;
        self.tapdrag_second_touch = tapdrag_second_touch;
        self.release_pending = false;
        self.down_ms = now_ms;
        self.release_pending_ms = 0;
        self.centroid_dx = 0;
        self.centroid_dy = 0;
        self.distance_delta = 0;
        self.centroid_last_x = metrics.centroid_x;
        self.centroid_last_y = metrics.centroid_y;
        self.distance_last = metrics.distance;
        self.metrics_stale = false;
        self.mode = TwoFingerMode::None;
        self.pinch_wheel_remainder = 0;
        self.pinch_button_sent = false;
    }

    fn start_relative_scroll(&mut self, now_ms: u32) {
        self.active = true;
        self.hold_sent = false;
        self.tap_candidate = false;
        self.hold_candidate = false;
        self.tapdrag_second_touch = false;
        self.release_pending = false;
        self.down_ms = now_ms;
        self.release_pending_ms = 0;
        self.centroid_dx = 0;
        self.centroid_dy = 0;
        self.distance_delta = 0;
        self.centroid_last_x = 0;
        self.centroid_last_y = 0;
        self.distance_last = 0;
        self.metrics_stale = true;
        self.mode = TwoFingerMode::Scroll;
        self.pinch_wheel_remainder = 0;
        self.pinch_button_sent = false;
    }

    fn sync_metrics(&mut self, metrics: TwoFingerMetrics) {
        self.centroid_last_x = metrics.centroid_x;
        self.centroid_last_y = metrics.centroid_y;
        self.distance_last = metrics.distance;
        self.metrics_stale = false;
    }

    fn mark_metrics_stale(&mut self) {
        self.metrics_stale = true;
    }

    fn update_metrics(&mut self, metrics: TwoFingerMetrics) -> TwoFingerStep {
        let step_x = metrics.centroid_x.saturating_sub(self.centroid_last_x);
        let step_y = metrics.centroid_y.saturating_sub(self.centroid_last_y);
        let step_distance = metrics.distance.saturating_sub(self.distance_last);

        self.centroid_last_x = metrics.centroid_x;
        self.centroid_last_y = metrics.centroid_y;
        self.distance_last = metrics.distance;
        self.centroid_dx = self.centroid_dx.saturating_add(step_x);
        self.centroid_dy = self.centroid_dy.saturating_add(step_y);
        self.distance_delta = self.distance_delta.saturating_add(step_distance);
        self.metrics_stale = false;

        TwoFingerStep {
            x: step_x,
            y: step_y,
            distance: step_distance,
        }
    }

    fn cancel_tap_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.tap_valid(now_ms, max_ms, move_threshold) {
            self.tap_candidate = false;
        }
    }

    fn cancel_hold_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.hold_valid(now_ms, max_ms, move_threshold) {
            self.hold_candidate = false;
        }
    }

    fn classify_mode(&mut self) {
        if self.mode == TwoFingerMode::Pinch {
            return;
        }

        let abs_center = abs_i32(self.centroid_dx).max(abs_i32(self.centroid_dy));
        let abs_distance = abs_i32(self.distance_delta);

        let scroll_dominates_distance =
            abs_center >= abs_distance.saturating_mul(TWO_FINGER_SCROLL_DOMINANCE_MULTIPLIER);

        if abs_distance >= TWO_FINGER_PINCH_START_DISTANCE && abs_distance > abs_center {
            self.mode = TwoFingerMode::Pinch;
            self.tap_candidate = false;
        } else if self.mode == TwoFingerMode::None
            && abs_center >= TWO_FINGER_SCROLL_START_MOVE
            && scroll_dominates_distance
        {
            self.mode = TwoFingerMode::Scroll;
            self.tap_candidate = false;
        }
    }

    fn pinch_wheel(&mut self, step_distance: i32) -> i16 {
        let divisor = TWO_FINGER_PINCH_WHEEL_DIVISOR.saturating_mul(10);
        let value = step_distance
            .saturating_mul(TWO_FINGER_PINCH_WHEEL_GAIN_X10)
            .saturating_add(self.pinch_wheel_remainder);
        let wheel = value / divisor;
        self.pinch_wheel_remainder = value.saturating_sub(wheel.saturating_mul(divisor));
        clamp_i32_to_i16(wheel)
    }

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
            && self.mode == TwoFingerMode::None
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.centroid_dx) <= i32::from(move_threshold)
            && abs_i32(self.centroid_dy) <= i32::from(move_threshold)
            && abs_i32(self.distance_delta) <= i32::from(move_threshold)
    }

    fn hold_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.hold_candidate
            && self.mode == TwoFingerMode::None
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.centroid_dx) <= i32::from(move_threshold)
            && abs_i32(self.centroid_dy) <= i32::from(move_threshold)
            && abs_i32(self.distance_delta) <= i32::from(move_threshold)
    }

    fn tap_lead_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_valid(now_ms, max_ms, move_threshold)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreeFingerState {
    active: bool,
    hold_sent: bool,
    tap_candidate: bool,
    hold_candidate: bool,
    tapdrag_second_touch: bool,
    release_pending: bool,
    down_ms: u32,
    release_pending_ms: u32,
    dx: i32,
    dy: i32,
    last_x: i32,
    last_y: i32,
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
    max_abs_dx: i32,
    max_abs_dy: i32,
    swipe_sent: bool,
}

impl ThreeFingerState {
    pub const fn new() -> Self {
        Self {
            active: false,
            hold_sent: false,
            tap_candidate: false,
            hold_candidate: false,
            tapdrag_second_touch: false,
            release_pending: false,
            down_ms: 0,
            release_pending_ms: 0,
            dx: 0,
            dy: 0,
            last_x: 0,
            last_y: 0,
            start_x: 0,
            start_y: 0,
            current_x: 0,
            current_y: 0,
            max_abs_dx: 0,
            max_abs_dy: 0,
            swipe_sent: false,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn start(
        &mut self,
        now_ms: u32,
        x: i32,
        y: i32,
        tap_candidate: bool,
        tapdrag_second_touch: bool,
    ) {
        self.active = true;
        self.hold_sent = tapdrag_second_touch;
        self.tap_candidate = tap_candidate;
        self.hold_candidate = tapdrag_second_touch;
        self.tapdrag_second_touch = tapdrag_second_touch;
        self.release_pending = false;
        self.down_ms = now_ms;
        self.release_pending_ms = 0;
        self.dx = 0;
        self.dy = 0;
        self.last_x = x;
        self.last_y = y;
        self.start_x = x;
        self.start_y = y;
        self.current_x = x;
        self.current_y = y;
        self.max_abs_dx = 0;
        self.max_abs_dy = 0;
        self.swipe_sent = false;
    }

    fn update_position(&mut self, x: i32, y: i32) {
        self.dx = self.dx.saturating_add(x.saturating_sub(self.last_x));
        self.dy = self.dy.saturating_add(y.saturating_sub(self.last_y));
        self.last_x = x;
        self.last_y = y;
        self.current_x = x;
        self.current_y = y;

        let dx = abs_i32(x.saturating_sub(self.start_x));
        let dy = abs_i32(y.saturating_sub(self.start_y));
        if dx > self.max_abs_dx {
            self.max_abs_dx = dx;
        }
        if dy > self.max_abs_dy {
            self.max_abs_dy = dy;
        }
    }

    fn cancel_tap_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.tap_valid(now_ms, max_ms, move_threshold) {
            self.tap_candidate = false;
        }
    }

    fn cancel_hold_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.hold_valid(now_ms, max_ms, move_threshold) {
            self.hold_candidate = false;
        }
    }

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.dx) <= i32::from(move_threshold)
            && abs_i32(self.dy) <= i32::from(move_threshold)
    }

    fn hold_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.hold_candidate
            && now_ms.wrapping_sub(self.down_ms) <= max_ms
            && abs_i32(self.dx) <= i32::from(move_threshold)
            && abs_i32(self.dy) <= i32::from(move_threshold)
    }

    fn swipe_button(self, threshold: u16) -> Option<TrackpadButton> {
        if self.max_abs_dx < i32::from(threshold) && self.max_abs_dy < i32::from(threshold) {
            return None;
        }

        if self.max_abs_dx >= self.max_abs_dy {
            if self.start_x <= self.current_x {
                Some(TrackpadButton::GestureRight)
            } else {
                Some(TrackpadButton::GestureLeft)
            }
        } else if self.start_y <= self.current_y {
            Some(TrackpadButton::GestureDown)
        } else {
            Some(TrackpadButton::GestureUp)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TwoFingerMetrics {
    centroid_x: i32,
    centroid_y: i32,
    distance: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FingerHistoryEntry {
    ms: u32,
    finger_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FingerHistory {
    entries: [FingerHistoryEntry; FINGER_HISTORY_SIZE],
    head: usize,
    count: usize,
}

const TAP_REENTRY_WINDOW_MS: u32 = 30;
const ONE_FINGER_TAPDRAG_GAP_MAX_MS: u32 = 160;
const TWO_FINGER_TAPDRAG_GAP_MAX_MS: u32 = 200;
const THREE_FINGER_TAPDRAG_GAP_MAX_MS: u32 = 200;
const TWO_FINGER_RELEASE_PENDING_MAX_MS: u32 = 150;
const THREE_FINGER_RELEASE_PENDING_MAX_MS: u32 = 150;
const TWO_FINGER_ONE_LEAD_MAX_MS: u32 = 120;
const THREE_FINGER_ONE_LEAD_MAX_MS: u32 = 120;
const THREE_FINGER_TWO_LEAD_MAX_MS: u32 = 120;
const TWO_FINGER_SCROLL_START_MOVE: i32 = 24;
const TWO_FINGER_RELATIVE_SCROLL_START_MOVE: i16 = 8;
const TWO_FINGER_SCROLL_DOMINANCE_MULTIPLIER: i32 = 2;
const TWO_FINGER_PINCH_START_DISTANCE: i32 = 100;
const TWO_FINGER_PINCH_WHEEL_DIVISOR: i32 = 96;
const TWO_FINGER_PINCH_WHEEL_GAIN_X10: i32 = 10;
const PINCH_WHEEL_MAX_STEP: i16 = 1;
const PINCH_KEY_SETTLE_MS: u32 = 20;
const FINGER_HISTORY_SIZE: usize = 5;

impl FingerHistory {
    pub const fn new() -> Self {
        Self {
            entries: [FingerHistoryEntry {
                ms: 0,
                finger_count: 0,
            }; FINGER_HISTORY_SIZE],
            head: 0,
            count: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn push(&mut self, finger_count: u8, now_ms: u32) {
        self.entries[self.head] = FingerHistoryEntry {
            ms: now_ms,
            finger_count,
        };
        self.head = (self.head + 1) % FINGER_HISTORY_SIZE;
        if self.count < FINGER_HISTORY_SIZE {
            self.count += 1;
        }
    }

    fn has_recent(self, finger_count: u8, now_ms: u32, window_ms: u32) -> bool {
        let mut i = 0;
        while i < self.count {
            let idx = (self.head + FINGER_HISTORY_SIZE - 1 - i) % FINGER_HISTORY_SIZE;
            let entry = self.entries[idx];
            if now_ms.wrapping_sub(entry.ms) > window_ms {
                break;
            }
            if entry.finger_count == finger_count {
                return true;
            }
            i += 1;
        }

        false
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
    pub const fn empty() -> Self {
        Self {
            relative_x: 0,
            relative_y: 0,
            single_gestures: 0,
            two_finger_gestures: 0,
            info_flags: 0,
            trackpad_flags: 0,
            finger1: FingerPosition { x: 0, y: 0 },
            finger2: FingerPosition { x: 0, y: 0 },
        }
    }

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

    pub const fn movement_detected(self) -> bool {
        self.trackpad_flags & TP_MOVEMENT_DETECTED != 0
    }

    pub const fn hardware_tap_gesture(self) -> Option<TrackpadGestureEvent> {
        if self.single_gestures & SFG_SINGLE_TAP != 0 {
            Some(TrackpadGestureEvent::Click(TrackpadButton::LeftClick))
        } else if self.two_finger_gestures & TFG_TWO_TAP != 0 {
            Some(TrackpadGestureEvent::Click(TrackpadButton::RightClick))
        } else {
            None
        }
    }
}

const fn read_u16_le(bytes: &[u8; COORD_BLOCK_LENGTH], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

const fn read_i16_le(bytes: &[u8; COORD_BLOCK_LENGTH], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn get_finger1_position(
    frame: CoordinateFrame,
    prev_frame: CoordinateFrame,
    axis_transform: TrackpadAxisTransform,
) -> Option<(i32, i32)> {
    if finger1_valid(frame) {
        return Some(
            axis_transform.apply((i32::from(frame.finger1.x), i32::from(frame.finger1.y))),
        );
    }

    if finger1_valid(prev_frame) {
        return Some(axis_transform.apply((
            i32::from(prev_frame.finger1.x),
            i32::from(prev_frame.finger1.y),
        )));
    }

    None
}

fn get_finger2_position(
    frame: CoordinateFrame,
    prev_frame: CoordinateFrame,
    axis_transform: TrackpadAxisTransform,
) -> Option<(i32, i32)> {
    if finger2_valid(frame) {
        return Some(
            axis_transform.apply((i32::from(frame.finger2.x), i32::from(frame.finger2.y))),
        );
    }

    if finger2_valid(prev_frame) {
        return Some(axis_transform.apply((
            i32::from(prev_frame.finger2.x),
            i32::from(prev_frame.finger2.y),
        )));
    }

    None
}

fn get_two_finger_metrics(
    frame: CoordinateFrame,
    prev_frame: CoordinateFrame,
    axis_transform: TrackpadAxisTransform,
) -> Option<TwoFingerMetrics> {
    let (f1x, f1y) = get_finger1_position(frame, prev_frame, axis_transform)?;
    let (f2x, f2y) = get_finger2_position(frame, prev_frame, axis_transform)?;

    Some(TwoFingerMetrics {
        centroid_x: (f1x + f2x) / 2,
        centroid_y: (f1y + f2y) / 2,
        distance: abs_i32(f1x.saturating_sub(f2x)) + abs_i32(f1y.saturating_sub(f2y)),
    })
}

fn two_finger_positions_confident(frame: CoordinateFrame) -> bool {
    frame.finger_count() == 2 && finger1_valid(frame) && finger2_valid(frame)
}

fn two_finger_relative_scroll_delta(
    frame: CoordinateFrame,
    axis_transform: TrackpadAxisTransform,
    min_move: i16,
) -> Option<TrackpadScrollDelta> {
    if frame.finger_count() != 2 || !frame.movement_detected() {
        return None;
    }

    let (x, y) = axis_transform.apply((i32::from(frame.relative_x), i32::from(frame.relative_y)));
    let x = clamp_i32_to_i16(x);
    let y = clamp_i32_to_i16(y);
    let min_move = min_move.max(0);
    if x == 0 && y == 0 {
        return None;
    }
    if min_move > 0 && x.saturating_abs().max(y.saturating_abs()) < min_move {
        return None;
    }

    Some(TrackpadScrollDelta { x, y })
}

fn finger1_valid(frame: CoordinateFrame) -> bool {
    frame.trackpad_flags & TP_FINGER1_CONFIDENCE != 0 && finger_position_valid(frame.finger1)
}

fn finger2_valid(frame: CoordinateFrame) -> bool {
    frame.trackpad_flags & TP_FINGER2_CONFIDENCE != 0 && finger_position_valid(frame.finger2)
}

const fn finger_position_valid(position: FingerPosition) -> bool {
    position.x != u16::MAX && position.y != u16::MAX
}

fn take_pending_tapdrag_second_touch(
    pending: &mut bool,
    pending_ms: &mut u32,
    now_ms: u32,
    max_gap_ms: u32,
) -> bool {
    if !*pending {
        return false;
    }

    let elapsed_ms = now_ms.wrapping_sub(*pending_ms);
    *pending = false;
    *pending_ms = 0;
    elapsed_ms <= max_gap_ms
}

fn pending_tapdrag_second_touch(
    pending: bool,
    pending_ms: u32,
    now_ms: u32,
    max_gap_ms: u32,
) -> bool {
    pending && now_ms.wrapping_sub(pending_ms) <= max_gap_ms
}

fn deferred_hold_expired(
    finger_count: u8,
    pending_finger_count: u8,
    elapsed_ms: u32,
    max_gap_ms: u32,
) -> bool {
    finger_count > pending_finger_count
        || (finger_count == pending_finger_count && elapsed_ms > max_gap_ms)
        || (finger_count == 0 && elapsed_ms >= max_gap_ms)
}

fn abs_i32(value: i32) -> i32 {
    if value < 0 {
        value.saturating_neg()
    } else {
        value
    }
}

fn clamp_i32_to_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn clamp_i16_to_i8(value: i16) -> i8 {
    value.clamp(i16::from(i8::MIN), i16::from(i8::MAX)) as i8
}

fn clamp_pending_motion(value: i16) -> i16 {
    value.clamp(-MAX_PENDING_MOTION, MAX_PENDING_MOTION)
}

fn clamp_pending_scroll(value: i16) -> i16 {
    value.clamp(-DEFAULT_SCROLL_MAX_STEP, DEFAULT_SCROLL_MAX_STEP)
}

fn clamp_scroll_step(value: i16, max_step: i16) -> i16 {
    let max_step = if max_step <= 0 { 1 } else { max_step };
    value.clamp(-max_step, max_step)
}

fn fixed_point_to_i16_rounded(value: i32, shift: u8) -> i16 {
    if shift == 0 {
        return clamp_i32_to_i16(value);
    }

    let half = 1_i64 << (shift - 1);
    let raw = i64::from(value);
    let magnitude = raw.abs();
    let rounded_magnitude = (magnitude.saturating_add(half)) >> shift;
    let rounded = if raw < 0 {
        -rounded_magnitude
    } else {
        rounded_magnitude
    };
    rounded.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

fn effective_scroll_divisor(
    delta: i16,
    divisor: u16,
    low_speed_divisor: u16,
    low_speed_threshold: i16,
) -> u16 {
    let threshold = low_speed_threshold.max(0);
    if threshold > 0 && delta != 0 && delta.saturating_abs() <= threshold {
        low_speed_divisor
    } else {
        divisor
    }
}

fn smooth_scroll_axis(previous_fp: i32, sample: i16) -> i32 {
    if sample == 0 {
        return 0;
    }

    let sample_fp = i32::from(sample) << SCROLL_SMOOTHING_FP_SHIFT;
    let total_weight = SCROLL_SMOOTHING_PREVIOUS_WEIGHT + SCROLL_SMOOTHING_CURRENT_WEIGHT;
    if total_weight <= 0 {
        return sample_fp;
    }

    (previous_fp
        .saturating_mul(SCROLL_SMOOTHING_PREVIOUS_WEIGHT)
        .saturating_add(sample_fp.saturating_mul(SCROLL_SMOOTHING_CURRENT_WEIGHT)))
        / total_weight
}

fn scale_with_remainder(value: i32, divisor: u16, remainder: &mut i32) -> i32 {
    let divisor = i32::from(if divisor == 0 { 1 } else { divisor });
    let total = value.saturating_add(*remainder);
    let scaled = total / divisor;
    *remainder = total - scaled * divisor;
    scaled
}

fn scale_scroll_axis_with_remainder(
    value: i32,
    divisor: u16,
    remainder: &mut i32,
    max_step: i16,
) -> i32 {
    if value == 0 {
        return 0;
    }

    scale_with_remainder_limited(value, divisor, remainder, max_step)
}

fn scale_with_remainder_limited(
    value: i32,
    divisor: u16,
    remainder: &mut i32,
    max_step: i16,
) -> i32 {
    let divisor = i32::from(if divisor == 0 { 1 } else { divisor });
    let max_step = i32::from(if max_step <= 0 { 1 } else { max_step });
    let total = value.saturating_add(*remainder);
    let scaled = total / divisor;
    *remainder = total - scaled * divisor;
    scaled.clamp(-max_step, max_step)
}

fn cursor_motion_detected(frame: CoordinateFrame) -> bool {
    frame.finger_count() == 1 && frame.movement_detected()
}

fn default_motion_interval() -> Option<Duration> {
    if DEFAULT_MOTION_INTERVAL_MS == 0 {
        None
    } else {
        Some(Duration::from_millis(DEFAULT_MOTION_INTERVAL_MS))
    }
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

    #[test]
    fn defers_left_button_release_for_one_finger_tap() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(1, 100, 200, 0, 0), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(1, 115, 210, 0, 0), 1050),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1100),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: true,
            })
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1259),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1260),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: false,
            })
        );
    }

    #[test]
    fn keeps_left_button_held_for_one_finger_tap_drag_reentry() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(1, 100, 200, 0, 0), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1060),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: true,
            })
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(1, 102, 201, 0, 0), 1120),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(1, 190, 240, 0, 0), 1180),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1240),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: false,
            })
        );
    }

    #[test]
    fn emits_right_click_for_two_finger_tap() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 200, 300, 200), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 110, 205, 310, 205), 1050),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1100),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::RightClick,
                pressed: true,
            })
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1300),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::RightClick,
                pressed: false,
            })
        );
    }

    #[test]
    fn emits_three_finger_swipe_once() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(3, 100, 200, 0, 0), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(3, 350, 210, 0, 0), 1050),
            Some(TrackpadGestureEvent::Click(TrackpadButton::GestureRight))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(3, 420, 210, 0, 0), 1100),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1150),
            None
        );
    }

    #[test]
    fn turns_gesture_click_into_press_and_release_events() {
        let events: std::vec::Vec<_> = TrackpadGestureEvent::Click(TrackpadButton::GestureUp)
            .button_events(TrackpadSide::Left)
            .collect();

        assert_eq!(
            events,
            std::vec![
                TrackpadButtonEvent {
                    button: TrackpadButton::GestureUp,
                    position: VirtualKeyPosition { row: 6, col: 2 },
                    pressed: true,
                },
                TrackpadButtonEvent {
                    button: TrackpadButton::GestureUp,
                    position: VirtualKeyPosition { row: 6, col: 2 },
                    pressed: false,
                },
            ]
        );
    }

    #[test]
    fn encodes_pointer_motion_as_custom_event() {
        let motion = TrackpadMotionEvent {
            side: TrackpadSide::Left,
            x: -12,
            y: 34,
            wheel: 3,
            pan: -4,
            buttons: 0,
            button_state_valid: false,
        };

        assert_eq!(TrackpadMotionEvent::decode(motion.encode()), Some(motion));
        assert_eq!(TrackpadMotionEvent::decode([0; 16]), None);

        let button_state = TrackpadMotionEvent::button_state(TrackpadSide::Right, 1);
        assert_eq!(
            TrackpadMotionEvent::decode(button_state.encode()),
            Some(button_state)
        );
    }

    #[test]
    fn decodes_dynamic_scale_custom_event() {
        let mut payload = custom_event_payload(CUSTOM_EVENT_DYNAMIC_SCALE);
        payload[5] = 2;
        payload[6] = 3;

        assert_eq!(
            TrackpadDynamicScaleEvent::decode(payload),
            Some(TrackpadDynamicScaleEvent::new(
                TrackpadDynamicScaleGroup::All,
                TrackpadDynamicScaleAction::Reset
            ))
        );
    }

    #[test]
    fn pinch_motion_reverses_direction_and_caps_wheel_step() {
        assert_eq!(
            pinch_wheel_to_motion(TrackpadSide::Right, 4),
            Some(TrackpadMotionEvent::scroll(TrackpadSide::Right, -1, 0))
        );
        assert_eq!(
            pinch_wheel_to_motion(TrackpadSide::Right, -4),
            Some(TrackpadMotionEvent::scroll(TrackpadSide::Right, 1, 0))
        );
    }

    #[test]
    fn applies_motion_transform_and_divisor() {
        let config = TrackpadMotionConfig::new(TrackpadAxisTransform::new(true, false, true), 2);
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.motion_event(
                TrackpadSide::Right,
                20,
                -8,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 4,
                y: 10,
                wheel: 0,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn accumulates_scaled_motion_remainders() {
        let config = TrackpadMotionConfig::new(TrackpadAxisTransform::default(), 3);
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.motion_event(
                TrackpadSide::Left,
                2,
                -2,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
        assert_eq!(
            config.motion_event(
                TrackpadSide::Left,
                1,
                -1,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Left,
                x: 1,
                y: -1,
                wheel: 0,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn scales_fast_two_finger_scroll_to_unit_wheel_and_pan() {
        let config = TrackpadScrollConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                64,
                -96,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 0,
                y: 0,
                wheel: 2,
                pan: 2,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn low_speed_scroll_uses_larger_divisor() {
        let config = TrackpadScrollConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -6,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -8,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -10,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 0,
                y: 0,
                wheel: 1,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn smooths_scroll_delta_spikes() {
        assert_eq!(
            fixed_point_to_i16_rounded(smooth_scroll_axis(0, 80), SCROLL_SMOOTHING_FP_SHIFT),
            40
        );
        assert_eq!(
            fixed_point_to_i16_rounded(
                smooth_scroll_axis(40 << SCROLL_SMOOTHING_FP_SHIFT, 0),
                SCROLL_SMOOTHING_FP_SHIFT
            ),
            0
        );
    }

    #[test]
    fn scroll_smoothing_preserves_small_delta() {
        assert_eq!(
            fixed_point_to_i16_rounded(smooth_scroll_axis(0, 1), SCROLL_SMOOTHING_FP_SHIFT),
            1
        );
    }

    #[test]
    fn rounds_negative_fixed_point_without_extra_bias() {
        assert_eq!(
            fixed_point_to_i16_rounded(
                -(1 << SCROLL_SMOOTHING_FP_SHIFT),
                SCROLL_SMOOTHING_FP_SHIFT
            ),
            -1
        );
        assert_eq!(
            fixed_point_to_i16_rounded(
                -(40 << SCROLL_SMOOTHING_FP_SHIFT),
                SCROLL_SMOOTHING_FP_SHIFT
            ),
            -40
        );
    }

    #[test]
    fn drops_excess_large_scroll_delta_after_capped_report() {
        let config = TrackpadScrollConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -120,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 0,
                y: 0,
                wheel: 2,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                0,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
    }

    #[test]
    fn stationary_scroll_frame_does_not_drain_low_speed_remainder() {
        let config = TrackpadScrollConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -10,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                -10,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
        assert_eq!(
            config.scroll_event(
                TrackpadSide::Right,
                0,
                0,
                &mut remainder_x,
                &mut remainder_y
            ),
            None
        );
    }

    #[test]
    fn default_motion_transform_keeps_xy_direction() {
        let config = TrackpadMotionConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.motion_event(
                TrackpadSide::Right,
                12,
                -7,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 12,
                y: -7,
                wheel: 0,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn emits_scroll_for_two_finger_centroid_motion() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 170, 200, 170), 1010),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 70,
            }))
        );
    }

    #[test]
    fn defers_initial_two_finger_relative_scroll_when_positions_are_confident() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 30, true, true),
                1000
            ),
            None
        );
    }

    #[test]
    fn emits_scroll_from_relative_motion_when_finger_positions_are_not_confident() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 12, false, false),
                1000
            ),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 12,
            }))
        );
    }

    #[test]
    fn ignores_relative_scroll_during_two_finger_tapdrag_reentry() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 200, 300, 200), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1060),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::RightClick,
                pressed: true,
            })
        );
        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 30, false, false),
                1120
            ),
            None
        );
    }

    #[test]
    fn ignores_tiny_relative_scroll_before_scroll_mode_is_selected() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 1, false, false),
                1010
            ),
            None
        );
    }

    #[test]
    fn emits_scroll_ended_after_relative_scroll_release() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 12, false, false),
                1000
            ),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 12,
            }))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1010),
            Some(TrackpadGestureEvent::ScrollEnded)
        );
    }

    #[test]
    fn continues_scroll_from_relative_motion_after_position_confidence_drops() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 10, false, false),
                1010
            ),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 10,
            }))
        );
    }

    #[test]
    fn prefers_relative_scroll_when_current_confidence_drops_but_prev_frame_was_valid() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 12, false, false),
                1010
            ),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 12,
            }))
        );
    }

    #[test]
    fn resyncs_absolute_metrics_after_relative_scroll_without_burst() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(
                frame_with_relative_two_finger_motion(0, 10, false, false),
                1010
            ),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 10,
            }))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 130, 200, 130), 1020),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 140, 200, 140), 1030),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 10,
            }))
        );
    }

    #[test]
    fn emits_scroll_ended_after_two_finger_scroll_release() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 170, 200, 170), 1010),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 0,
                y: 70,
            }))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1020),
            Some(TrackpadGestureEvent::ScrollEnded)
        );
    }

    #[test]
    fn emits_pinch_button_and_wheel_for_two_finger_distance_motion() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 20, 100, 280, 100), 1010),
            Some(TrackpadGestureEvent::PinchStarted(1))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 0, 100, 300, 100), 1020),
            Some(TrackpadGestureEvent::PinchWheel(1))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1030),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::Pinch,
                pressed: false,
            })
        );
    }

    #[test]
    fn prefers_pinch_over_scroll_when_distance_change_dominates() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 80, 100, 340, 100), 1010),
            Some(TrackpadGestureEvent::PinchStarted(1))
        );
    }

    #[test]
    fn keeps_ambiguous_two_finger_motion_pending_until_pinch_threshold() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 110, 100, 290, 100), 1010),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 80, 100, 330, 100), 1020),
            Some(TrackpadGestureEvent::PinchStarted(0))
        );
    }

    #[test]
    fn switches_from_scroll_to_pinch_when_distance_becomes_dominant() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();

        assert_eq!(
            recognizer.update(frame_with_fingers(2, 100, 100, 200, 100), 1000),
            None
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 160, 100, 240, 100), 1010),
            Some(TrackpadGestureEvent::Scroll(TrackpadScrollDelta {
                x: 50,
                y: 0,
            }))
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(2, 120, 100, 380, 100), 1020),
            Some(TrackpadGestureEvent::PinchStarted(1))
        );
    }

    #[test]
    fn seeds_scroll_inertia_from_recent_history() {
        let mut history = ScrollMotionHistory::new();
        history.push(TrackpadScrollDelta { x: 0, y: 60 }, 1000);
        history.push(TrackpadScrollDelta { x: 0, y: 60 }, 1010);

        let seed = history
            .seed(1020)
            .expect("recent scroll should seed inertia");
        assert_eq!(seed.vx_fp, 0);
        assert!(seed.vy_fp > 0);
    }

    #[test]
    fn seeds_cursor_inertia_from_fast_recent_motion() {
        let mut history = ScrollMotionHistory::new();
        history.push(TrackpadScrollDelta { x: 12, y: 0 }, 1000);
        history.push(TrackpadScrollDelta { x: 12, y: 0 }, 1010);

        let seed = history
            .seed_with(1020, CURSOR_INERTIA_CONFIG)
            .expect("recent cursor motion should seed inertia");
        assert!(seed.vx_fp > 0);
        assert_eq!(seed.vy_fp, 0);
    }

    #[test]
    fn scroll_inertia_decays_after_release() {
        let mut history = ScrollMotionHistory::new();
        history.push(TrackpadScrollDelta { x: 0, y: 60 }, 1000);
        history.push(TrackpadScrollDelta { x: 0, y: 60 }, 1010);
        let seed = history
            .seed(1020)
            .expect("recent scroll should seed inertia");

        let mut inertia = ScrollInertiaState::new();
        inertia.start(seed, 1020);
        assert_eq!(inertia.step(1025), None);

        let first = inertia.step(1030).expect("due inertia step");
        assert_eq!(first.x, 0);
        assert!(first.y > 0);

        let second = inertia.step(1040).expect("second inertia step");
        assert!(second.y > 0);
        assert!(second.y <= first.y);
    }

    #[test]
    fn inertia_scroll_uses_lower_sensitivity_and_step_cap() {
        let config = TrackpadScrollConfig::default();
        let mut remainder_x = 0;
        let mut remainder_y = 0;

        assert_eq!(
            config.inertia_scroll_event(
                TrackpadSide::Right,
                0,
                -120,
                &mut remainder_x,
                &mut remainder_y
            ),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 0,
                y: 0,
                wheel: 1,
                pan: 0,
                buttons: 0,
                button_state_valid: false,
            })
        );
    }

    #[test]
    fn cursor_motion_does_not_cancel_tap_by_itself() {
        let mut recognizer = TrackpadGestureRecognizer::with_defaults();
        let motion_frame = CoordinateFrame {
            relative_x: 1,
            trackpad_flags: TP_MOVEMENT_DETECTED | TP_FINGER1_CONFIDENCE | u16::from(1u8),
            finger1: FingerPosition { x: 100, y: 200 },
            ..CoordinateFrame::default()
        };

        assert_eq!(recognizer.update(motion_frame, 1000), None);
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1050),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: true,
            })
        );
        assert_eq!(
            recognizer.update(frame_with_fingers(0, 0, 0, 0, 0), 1210),
            Some(TrackpadGestureEvent::Button {
                button: TrackpadButton::LeftClick,
                pressed: false,
            })
        );
    }

    #[test]
    fn ignores_relative_delta_without_movement_flag() {
        assert!(cursor_motion_detected(CoordinateFrame {
            relative_x: 2,
            trackpad_flags: TP_MOVEMENT_DETECTED | 1,
            ..CoordinateFrame::default()
        }));
        assert!(!cursor_motion_detected(CoordinateFrame {
            relative_x: 2,
            trackpad_flags: 0,
            ..CoordinateFrame::default()
        }));
    }

    fn frame_with_fingers(
        fingers: u8,
        finger1_x: u16,
        finger1_y: u16,
        finger2_x: u16,
        finger2_y: u16,
    ) -> CoordinateFrame {
        let mut trackpad_flags = u16::from(fingers);
        if fingers >= 1 {
            trackpad_flags |= TP_FINGER1_CONFIDENCE;
        }
        if fingers >= 2 {
            trackpad_flags |= TP_FINGER2_CONFIDENCE;
        }

        CoordinateFrame {
            trackpad_flags,
            finger1: FingerPosition {
                x: finger1_x,
                y: finger1_y,
            },
            finger2: FingerPosition {
                x: finger2_x,
                y: finger2_y,
            },
            ..CoordinateFrame::default()
        }
    }

    fn frame_with_relative_two_finger_motion(
        relative_x: i16,
        relative_y: i16,
        finger1_confident: bool,
        finger2_confident: bool,
    ) -> CoordinateFrame {
        let mut trackpad_flags = TP_MOVEMENT_DETECTED | 2;
        if finger1_confident {
            trackpad_flags |= TP_FINGER1_CONFIDENCE;
        }
        if finger2_confident {
            trackpad_flags |= TP_FINGER2_CONFIDENCE;
        }

        CoordinateFrame {
            relative_x,
            relative_y,
            trackpad_flags,
            finger1: if finger1_confident {
                FingerPosition { x: 100, y: 100 }
            } else {
                FingerPosition {
                    x: u16::MAX,
                    y: u16::MAX,
                }
            },
            finger2: if finger2_confident {
                FingerPosition { x: 200, y: 100 }
            } else {
                FingerPosition {
                    x: u16::MAX,
                    y: u16::MAX,
                }
            },
            ..CoordinateFrame::default()
        }
    }
}
