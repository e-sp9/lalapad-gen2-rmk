//! IQS9151 porting helpers.
//!
//! The upstream ZMK driver emits `INPUT_BTN_0..7` for trackpad clicks and
//! gestures, then maps those events to virtual key positions. This module keeps
//! the RMK-side mapping and frame layout explicit while the runtime I2C driver
//! is being ported.

use core::future::Future;

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
    motion_remainder_x: i32,
    motion_remainder_y: i32,
    pending_click: Option<TrackpadClickEvents>,
    pending_motion: Option<TrackpadMotionEvent>,
    poll_interval: Duration,
    motion_interval: Option<Duration>,
    init_failure_count: u8,
    read_error_count: u8,
    degraded_mode: bool,
    diagnostic_motion_last_ms: u32,
    diagnostic_motion_sign: i16,
}

impl<I2C> Iqs9151InputDevice<I2C, NoReadyPin>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C, side: TrackpadSide) -> Self {
        Self::from_sensor(Iqs9151::new(i2c), side)
    }

    pub fn from_sensor(sensor: Iqs9151<I2C>, side: TrackpadSide) -> Self {
        Self {
            sensor,
            ready: NoReadyPin,
            side,
            motion_output: Iqs9151MotionOutput::RmkEvent,
            initialized: false,
            recognizer: TrackpadGestureRecognizer::with_defaults(),
            motion_config: TrackpadMotionConfig::default(),
            motion_remainder_x: 0,
            motion_remainder_y: 0,
            pending_click: None,
            pending_motion: None,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            motion_interval: default_motion_interval(),
            init_failure_count: 0,
            read_error_count: 0,
            degraded_mode: false,
            diagnostic_motion_last_ms: 0,
            diagnostic_motion_sign: 1,
        }
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
            motion_remainder_x: 0,
            motion_remainder_y: 0,
            pending_click: None,
            pending_motion: None,
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

    pub fn set_gesture_config(&mut self, config: TrackpadGestureConfig) {
        self.recognizer = TrackpadGestureRecognizer::new(config);
    }

    pub fn set_motion_config(&mut self, config: TrackpadMotionConfig) {
        self.motion_config = config;
        self.motion_remainder_x = 0;
        self.motion_remainder_y = 0;
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

    fn handle_motion(&mut self, motion: TrackpadMotionEvent) -> Option<Event> {
        let motion = motion.capped();
        match self.motion_output {
            Iqs9151MotionOutput::RmkEvent => Some(motion.into_rmk_event()),
            Iqs9151MotionOutput::HidReport => {
                if !send_mouse_motion_reports(motion) {
                    self.queue_pending_motion(motion);
                }
                None
            }
            Iqs9151MotionOutput::SplitEvent => {
                if !send_split_motion_event(motion) {
                    self.queue_pending_motion(motion);
                }
                None
            }
        }
    }

    fn flush_pending_motion(&mut self) {
        let Some(motion) = self.pending_motion else {
            return;
        };

        let sent = match self.motion_output {
            Iqs9151MotionOutput::RmkEvent => false,
            Iqs9151MotionOutput::HidReport => send_mouse_motion_reports(motion),
            Iqs9151MotionOutput::SplitEvent => send_split_motion_event(motion),
        };
        if sent {
            self.pending_motion = None;
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
                        self.recognizer.reset();
                        self.pending_motion = None;
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

            if let Some(events) = self.pending_click.as_mut() {
                if let Some(event) = events.next() {
                    return event.into_rmk_event();
                }
                self.pending_click = None;
            }

            self.flush_pending_motion();
            self.wait_ready_for(READY_RUNTIME_TIMEOUT_MS).await;

            match self.sensor.read_coordinate_frame().await {
                Ok(frame) => {
                    self.read_error_count = 0;
                    if frame.show_reset() {
                        self.recognizer.reset();
                        self.pending_motion = None;
                        self.motion_remainder_x = 0;
                        self.motion_remainder_y = 0;
                        self.initialized = false;
                        self.degraded_mode = false;
                        continue;
                    }

                    let now_ms = Instant::now().as_millis() as u32;
                    if let Some(gesture) = self.recognizer.update(frame, now_ms) {
                        self.pending_click = Some(gesture.button_events(self.side));
                        continue;
                    }

                    if !self.recognizer.cursor_suppressed() {
                        if let Some(motion) = self.motion_from_frame(frame) {
                            if let Some(event) = self.handle_motion(motion) {
                                return event;
                            }
                            if self.motion_interval.is_some() {
                                self.wait_motion_interval().await;
                            }
                            continue;
                        }
                    }

                    if frame.finger_count() == 0 {
                        self.motion_remainder_x = 0;
                        self.motion_remainder_y = 0;
                    }
                }
                Err(_) => {
                    self.read_error_count = self.read_error_count.saturating_add(1);
                    self.recognizer.reset();
                    self.motion_remainder_x = 0;
                    self.motion_remainder_y = 0;
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
            Event::Custom(payload) => match self.target {
                Iqs9151ControllerTarget::HidReport => {
                    if let Some(motion) = TrackpadMotionEvent::decode(payload) {
                        send_mouse_motion_reports(motion);
                    }
                }
                Iqs9151ControllerTarget::SplitEvent => {
                    if let Some(motion) = TrackpadMotionEvent::decode(payload) {
                        send_split_motion_event(motion);
                    }
                }
            },
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
            if let Some(motion) = TrackpadMotionEvent::decode(payload) {
                send_mouse_motion_reports(motion);
            }
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        EVENT_CHANNEL.receive().await
    }
}

fn send_mouse_motion_reports(mut motion: TrackpadMotionEvent) -> bool {
    motion = motion.capped();
    if motion.x == 0 && motion.y == 0 {
        return true;
    }

    let report = MouseReport {
        buttons: 0,
        x: clamp_i16_to_i8(motion.x),
        y: clamp_i16_to_i8(motion.y),
        wheel: 0,
        pan: 0,
    };

    KEYBOARD_REPORT_CHANNEL
        .try_send(Report::MouseReport(report))
        .is_ok()
}

fn send_split_motion_event(motion: TrackpadMotionEvent) -> bool {
    let motion = motion.capped();
    if motion.x == 0 && motion.y == 0 {
        return true;
    }
    if EVENT_CHANNEL.is_full() {
        let _ = EVENT_CHANNEL.try_receive();
    }
    EVENT_CHANNEL
        .try_send(Event::Custom(motion.encode()))
        .is_ok()
}

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
            Some(TrackpadMotionEvent { side, x, y })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackpadMotionEvent {
    pub side: TrackpadSide,
    pub x: i16,
    pub y: i16,
}

impl TrackpadMotionEvent {
    pub fn into_rmk_event(self) -> Event {
        Event::Custom(self.encode())
    }

    pub fn encode(self) -> [u8; 16] {
        let mut payload = [0u8; 16];
        payload[0..4].copy_from_slice(&CUSTOM_EVENT_PREFIX);
        payload[4] = CUSTOM_EVENT_POINTER_MOTION;
        payload[5] = match self.side {
            TrackpadSide::Left => 0,
            TrackpadSide::Right => 1,
        };
        payload[6..8].copy_from_slice(&self.x.to_le_bytes());
        payload[8..10].copy_from_slice(&self.y.to_le_bytes());
        payload
    }

    pub fn decode(payload: [u8; 16]) -> Option<Self> {
        if payload[0..4] != CUSTOM_EVENT_PREFIX || payload[4] != CUSTOM_EVENT_POINTER_MOTION {
            return None;
        }

        let side = match payload[5] {
            0 => TrackpadSide::Left,
            1 => TrackpadSide::Right,
            _ => return None,
        };
        let x = i16::from_le_bytes([payload[6], payload[7]]);
        let y = i16::from_le_bytes([payload[8], payload[9]]);

        Some(Self { side, x, y })
    }

    pub fn merge(self, next: Self) -> Self {
        if self.side != next.side {
            return next.capped();
        }

        Self {
            side: self.side,
            x: clamp_pending_motion(self.x.saturating_add(next.x)),
            y: clamp_pending_motion(self.y.saturating_add(next.y)),
        }
    }

    pub fn capped(self) -> Self {
        Self {
            side: self.side,
            x: clamp_pending_motion(self.x),
            y: clamp_pending_motion(self.y),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackpadGestureEvent {
    Click(TrackpadButton),
}

impl TrackpadGestureEvent {
    pub fn button_events(self, side: TrackpadSide) -> TrackpadClickEvents {
        match self {
            Self::Click(button) => TrackpadClickEvents::new(side, button),
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
    }

    pub const fn cursor_suppressed(self) -> bool {
        self.suppress_cursor_tail
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

        if finger_count == 3 && self.one_finger.active {
            self.three_finger_one_lead_valid = self.one_finger.tap_lead_valid(
                now_ms,
                THREE_FINGER_ONE_LEAD_MAX_MS,
                self.config.one_finger_tap_move,
            );
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
            self.two_finger.reset();
        } else {
            self.three_finger_two_lead_valid = false;
        }

        if finger_count == 2 && self.one_finger.active {
            self.two_finger_one_lead_valid = self.one_finger.tap_lead_valid(
                now_ms,
                TWO_FINGER_ONE_LEAD_MAX_MS,
                self.config.one_finger_tap_move,
            );
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
            1 => self.update_one_finger(frame, prev_frame, now_ms),
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

            self.one_finger.start(
                now_ms,
                x,
                y,
                prev_frame.finger_count() == 0
                    || self
                        .finger_history
                        .has_recent(0, now_ms, TAP_REENTRY_WINDOW_MS),
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
            return None;
        }

        let event = if frame.finger_count() == 0
            && self.one_finger.tap_valid(
                now_ms,
                self.config.one_finger_tap_max_ms,
                self.config.one_finger_tap_move,
            ) {
            Some(TrackpadGestureEvent::Click(TrackpadButton::LeftClick))
        } else {
            None
        };
        self.one_finger.reset();
        event
    }

    fn update_two_finger(
        &mut self,
        frame: CoordinateFrame,
        prev_frame: CoordinateFrame,
        now_ms: u32,
    ) -> Option<TrackpadGestureEvent> {
        let two_now = frame.finger_count() == 2;
        let have_xy = two_now
            .then(|| get_two_finger_metrics(frame, prev_frame, self.config.axis_transform))
            .flatten();

        if !self.two_finger.active && two_now {
            let Some(metrics) = have_xy else {
                return None;
            };

            self.two_finger.start(
                now_ms,
                metrics,
                prev_frame.finger_count() == 0
                    || self
                        .finger_history
                        .has_recent(0, now_ms, TAP_REENTRY_WINDOW_MS)
                    || self.two_finger_one_lead_valid,
            );
            self.two_finger_one_lead_valid = false;
            return None;
        }

        if !self.two_finger.active {
            self.two_finger_one_lead_valid = false;
            return None;
        }

        self.two_finger_one_lead_valid = false;

        if two_now {
            if let Some(metrics) = have_xy {
                self.two_finger.update_metrics(metrics);
            }
            self.two_finger.cancel_tap_if_needed(
                now_ms,
                self.config.two_finger_tap_max_ms,
                self.config.two_finger_tap_move,
            );
            self.two_finger.classify_mode();
            self.two_finger.release_pending = false;
            return None;
        }

        if self.two_finger.release_pending {
            let pending_ms = now_ms.wrapping_sub(self.two_finger.release_pending_ms);
            if frame.finger_count() == 1 && pending_ms <= TWO_FINGER_RELEASE_PENDING_MAX_MS {
                self.suppress_cursor_tail = true;
                return None;
            }

            let event = if frame.finger_count() == 0
                && pending_ms <= TWO_FINGER_RELEASE_PENDING_MAX_MS
                && self.two_finger.tap_valid(
                    now_ms,
                    self.config.two_finger_tap_max_ms,
                    self.config.two_finger_tap_move,
                ) {
                Some(TrackpadGestureEvent::Click(TrackpadButton::RightClick))
            } else {
                None
            };
            self.two_finger.reset();
            return event;
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
                    return Some(TrackpadGestureEvent::Click(TrackpadButton::RightClick));
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

            self.three_finger.start(
                now_ms,
                x,
                y,
                prev_frame.finger_count() == 0
                    || self
                        .finger_history
                        .has_recent(0, now_ms, TAP_REENTRY_WINDOW_MS)
                    || self.three_finger_one_lead_valid
                    || self.three_finger_two_lead_valid,
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
            if !self.three_finger.swipe_sent {
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

        let event = if frame.finger_count() == 0
            && !self.three_finger.swipe_sent
            && self.three_finger.tap_valid(
                now_ms,
                self.config.three_finger_tap_max_ms,
                self.config.three_finger_tap_move,
            ) {
            Some(TrackpadGestureEvent::Click(TrackpadButton::MiddleClick))
        } else {
            None
        };
        self.three_finger.reset();
        event
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
    tap_candidate: bool,
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
            tap_candidate: false,
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

    fn start(&mut self, now_ms: u32, x: i32, y: i32, tap_candidate: bool) {
        self.active = true;
        self.tap_candidate = tap_candidate;
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

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TwoFingerState {
    active: bool,
    tap_candidate: bool,
    release_pending: bool,
    down_ms: u32,
    release_pending_ms: u32,
    centroid_dx: i32,
    centroid_dy: i32,
    distance_delta: i32,
    centroid_last_x: i32,
    centroid_last_y: i32,
    distance_last: i32,
    mode: TwoFingerMode,
}

impl TwoFingerState {
    pub const fn new() -> Self {
        Self {
            active: false,
            tap_candidate: false,
            release_pending: false,
            down_ms: 0,
            release_pending_ms: 0,
            centroid_dx: 0,
            centroid_dy: 0,
            distance_delta: 0,
            centroid_last_x: 0,
            centroid_last_y: 0,
            distance_last: 0,
            mode: TwoFingerMode::None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn start(&mut self, now_ms: u32, metrics: TwoFingerMetrics, tap_candidate: bool) {
        self.active = true;
        self.tap_candidate = tap_candidate;
        self.release_pending = false;
        self.down_ms = now_ms;
        self.release_pending_ms = 0;
        self.centroid_dx = 0;
        self.centroid_dy = 0;
        self.distance_delta = 0;
        self.centroid_last_x = metrics.centroid_x;
        self.centroid_last_y = metrics.centroid_y;
        self.distance_last = metrics.distance;
        self.mode = TwoFingerMode::None;
    }

    fn update_metrics(&mut self, metrics: TwoFingerMetrics) {
        let step_x = metrics.centroid_x.saturating_sub(self.centroid_last_x);
        let step_y = metrics.centroid_y.saturating_sub(self.centroid_last_y);
        let step_distance = metrics.distance.saturating_sub(self.distance_last);

        self.centroid_last_x = metrics.centroid_x;
        self.centroid_last_y = metrics.centroid_y;
        self.distance_last = metrics.distance;
        self.centroid_dx = self.centroid_dx.saturating_add(step_x);
        self.centroid_dy = self.centroid_dy.saturating_add(step_y);
        self.distance_delta = self.distance_delta.saturating_add(step_distance);
    }

    fn cancel_tap_if_needed(&mut self, now_ms: u32, max_ms: u32, move_threshold: u16) {
        if !self.tap_valid(now_ms, max_ms, move_threshold) {
            self.tap_candidate = false;
        }
    }

    fn classify_mode(&mut self) {
        if self.mode != TwoFingerMode::None {
            return;
        }

        let abs_center = abs_i32(self.centroid_dx).max(abs_i32(self.centroid_dy));
        let abs_distance = abs_i32(self.distance_delta);

        if abs_center >= TWO_FINGER_SCROLL_START_MOVE {
            self.mode = TwoFingerMode::Scroll;
            self.tap_candidate = false;
        } else if abs_distance >= TWO_FINGER_PINCH_START_DISTANCE && abs_distance > abs_center {
            self.mode = TwoFingerMode::Pinch;
            self.tap_candidate = false;
        }
    }

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
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
    tap_candidate: bool,
    down_ms: u32,
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
            tap_candidate: false,
            down_ms: 0,
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

    fn start(&mut self, now_ms: u32, x: i32, y: i32, tap_candidate: bool) {
        self.active = true;
        self.tap_candidate = tap_candidate;
        self.down_ms = now_ms;
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

    fn tap_valid(self, now_ms: u32, max_ms: u32, move_threshold: u16) -> bool {
        self.tap_candidate
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
const TWO_FINGER_RELEASE_PENDING_MAX_MS: u32 = 150;
const TWO_FINGER_ONE_LEAD_MAX_MS: u32 = 120;
const THREE_FINGER_ONE_LEAD_MAX_MS: u32 = 120;
const THREE_FINGER_TWO_LEAD_MAX_MS: u32 = 120;
const TWO_FINGER_SCROLL_START_MOVE: i32 = 50;
const TWO_FINGER_PINCH_START_DISTANCE: i32 = 100;
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

fn finger1_valid(frame: CoordinateFrame) -> bool {
    frame.trackpad_flags & TP_FINGER1_CONFIDENCE != 0 && finger_position_valid(frame.finger1)
}

fn finger2_valid(frame: CoordinateFrame) -> bool {
    frame.trackpad_flags & TP_FINGER2_CONFIDENCE != 0 && finger_position_valid(frame.finger2)
}

const fn finger_position_valid(position: FingerPosition) -> bool {
    position.x != u16::MAX && position.y != u16::MAX
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

fn clamp_i16_to_i8(value: i16) -> i8 {
    value.clamp(i16::from(i8::MIN), i16::from(i8::MAX)) as i8
}

fn clamp_pending_motion(value: i16) -> i16 {
    value.clamp(-MAX_PENDING_MOTION, MAX_PENDING_MOTION)
}

fn scale_with_remainder(value: i32, divisor: u16, remainder: &mut i32) -> i32 {
    let divisor = i32::from(if divisor == 0 { 1 } else { divisor });
    let total = value.saturating_add(*remainder);
    let scaled = total / divisor;
    *remainder = total - scaled * divisor;
    scaled
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
    fn emits_left_click_for_one_finger_tap() {
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
            Some(TrackpadGestureEvent::Click(TrackpadButton::LeftClick))
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
            Some(TrackpadGestureEvent::Click(TrackpadButton::RightClick))
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
        };

        assert_eq!(TrackpadMotionEvent::decode(motion.encode()), Some(motion));
        assert_eq!(TrackpadMotionEvent::decode([0; 16]), None);
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
            })
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
            Some(TrackpadGestureEvent::Click(TrackpadButton::LeftClick))
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
}
