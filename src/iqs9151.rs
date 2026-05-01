//! IQS9151 porting helpers.
//!
//! The upstream ZMK driver emits `INPUT_BTN_0..7` for trackpad clicks and
//! gestures, then maps those events to virtual key positions. This module keeps
//! the RMK-side mapping and frame layout explicit while the runtime I2C driver
//! is being ported.

use core::future::Future;

use embassy_time::{Duration, Instant, Timer};
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

pub const COORD_BLOCK_START: u16 = ADDR_RELATIVE_X;
pub const COORD_BLOCK_LENGTH: usize = 0x48;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 10;

pub const INFO_SHOW_RESET: u16 = 1 << 7;
pub const INFO_GLOBAL_TP_TOUCH: u16 = 1 << 9;
pub const INFO_TP_TOUCH_TOGGLED: u16 = 1 << 13;

pub const TP_MOVEMENT_DETECTED: u16 = 1 << 4;
pub const TP_FINGER_COUNT_MASK: u16 = 0x000f;
pub const TP_FINGER1_CONFIDENCE: u16 = 1 << 8;
pub const TP_FINGER2_CONFIDENCE: u16 = 1 << 9;

const CUSTOM_EVENT_PREFIX: [u8; 4] = *b"IQSP";
const CUSTOM_EVENT_POINTER_MOTION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Iqs9151Error<E> {
    Bus(E),
    UnexpectedProductNumber(u16),
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoReadyPin;

impl Iqs9151Ready for NoReadyPin {
    async fn wait_ready(&mut self) {}
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
}

pub struct Iqs9151InputDevice<I2C, RDY = NoReadyPin> {
    sensor: Iqs9151<I2C>,
    ready: RDY,
    side: TrackpadSide,
    recognizer: TrackpadGestureRecognizer,
    motion_config: TrackpadMotionConfig,
    pending_click: Option<TrackpadClickEvents>,
    poll_interval: Duration,
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
            recognizer: TrackpadGestureRecognizer::with_defaults(),
            motion_config: TrackpadMotionConfig::default(),
            pending_click: None,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
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
            recognizer: TrackpadGestureRecognizer::with_defaults(),
            motion_config: TrackpadMotionConfig::default(),
            pending_click: None,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
        }
    }

    pub fn release(self) -> (I2C, RDY) {
        (self.sensor.release(), self.ready)
    }

    pub fn set_poll_interval(&mut self, poll_interval: Duration) {
        self.poll_interval = poll_interval;
    }

    pub fn set_gesture_config(&mut self, config: TrackpadGestureConfig) {
        self.recognizer = TrackpadGestureRecognizer::new(config);
    }

    pub fn set_motion_config(&mut self, config: TrackpadMotionConfig) {
        self.motion_config = config;
    }

    pub async fn verify_product_number(&mut self) -> Result<(), Iqs9151Error<I2C::Error>> {
        self.sensor.verify_product_number().await
    }

    fn motion_event(&self, frame: CoordinateFrame) -> Option<Event> {
        if frame.finger_count() == 0 {
            return None;
        }

        let motion =
            self.motion_config
                .motion_event(self.side, frame.relative_x, frame.relative_y)?;
        Some(motion.into_rmk_event())
    }
}

impl<I2C, RDY> InputDevice for Iqs9151InputDevice<I2C, RDY>
where
    I2C: I2c,
    RDY: Iqs9151Ready,
{
    async fn read_event(&mut self) -> Event {
        loop {
            if let Some(events) = self.pending_click.as_mut() {
                if let Some(event) = events.next() {
                    return event.into_rmk_event();
                }
                self.pending_click = None;
            }

            self.ready.wait_ready().await;

            match self.sensor.read_coordinate_frame().await {
                Ok(frame) => {
                    if frame.show_reset() {
                        self.recognizer.reset();
                    }

                    let now_ms = Instant::now().as_millis() as u32;
                    if let Some(gesture) = self.recognizer.update(frame, now_ms) {
                        self.pending_click = Some(gesture.button_events(self.side));
                        continue;
                    }

                    if let Some(event) = self.motion_event(frame) {
                        Timer::after(self.poll_interval).await;
                        return event;
                    }
                }
                Err(_) => {
                    self.recognizer.reset();
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
                        send_mouse_motion_report(motion).await;
                    }
                }
                Iqs9151ControllerTarget::SplitEvent => {
                    let _ = EVENT_CHANNEL.try_send(Event::Custom(payload));
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

pub struct Iqs9151SplitEventController;

impl Iqs9151SplitEventController {
    pub const fn new() -> Self {
        Self
    }
}

impl Controller for Iqs9151SplitEventController {
    type Event = Event;

    async fn process_event(&mut self, event: Self::Event) {
        if let Event::Custom(payload) = event
            && let Some(motion) = TrackpadMotionEvent::decode(payload)
        {
            send_mouse_motion_report(motion).await;
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        EVENT_CHANNEL.receive().await
    }
}

async fn send_mouse_motion_report(motion: TrackpadMotionEvent) {
    let report = MouseReport {
        buttons: 0,
        x: clamp_i16_to_i8(motion.x),
        y: clamp_i16_to_i8(motion.y),
        wheel: 0,
        pan: 0,
    };
    let _ = KEYBOARD_REPORT_CHANNEL.try_send(Report::MouseReport(report));
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
            axis_transform: TrackpadAxisTransform::default(),
            divisor: 1,
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
    ) -> Option<TrackpadMotionEvent> {
        if relative_x == 0 && relative_y == 0 {
            return None;
        }

        let (x, y) = self
            .axis_transform
            .apply((i32::from(relative_x), i32::from(relative_y)));
        let divisor = i32::from(if self.divisor == 0 { 1 } else { self.divisor });
        let x = clamp_i32_to_i16(x / divisor);
        let y = clamp_i32_to_i16(y / divisor);

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
    session: Option<GestureSession>,
}

impl TrackpadGestureRecognizer {
    pub const fn new(config: TrackpadGestureConfig) -> Self {
        Self {
            config,
            session: None,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(TrackpadGestureConfig::default())
    }

    pub const fn config(&self) -> TrackpadGestureConfig {
        self.config
    }

    pub fn reset(&mut self) {
        self.session = None;
    }

    pub fn update(&mut self, frame: CoordinateFrame, now_ms: u32) -> Option<TrackpadGestureEvent> {
        let finger_count = frame.finger_count();

        if finger_count == 0 {
            return self.finish_session(now_ms);
        }

        if finger_count > 3 {
            self.session = Some(GestureSession::start(
                finger_count,
                frame,
                now_ms,
                self.config.axis_transform,
            ));
            return None;
        }

        match self.session.as_mut() {
            Some(session) if session.fingers == finger_count => {
                session.update_position(frame, self.config.axis_transform);

                if finger_count == 3 && !session.swipe_sent {
                    if let Some(button) = session.swipe_button(self.config.three_finger_swipe_move)
                    {
                        session.swipe_sent = true;
                        return Some(TrackpadGestureEvent::Click(button));
                    }
                }
            }
            _ => {
                self.session = Some(GestureSession::start(
                    finger_count,
                    frame,
                    now_ms,
                    self.config.axis_transform,
                ));
            }
        }

        None
    }

    fn finish_session(&mut self, now_ms: u32) -> Option<TrackpadGestureEvent> {
        let session = self.session.take()?;
        if session.swipe_sent {
            return None;
        }

        let elapsed_ms = now_ms.wrapping_sub(session.start_ms);
        let button = match session.fingers {
            1 if elapsed_ms <= self.config.one_finger_tap_max_ms
                && session.max_movement() <= self.config.one_finger_tap_move =>
            {
                TrackpadButton::LeftClick
            }
            2 if elapsed_ms <= self.config.two_finger_tap_max_ms
                && session.max_movement() <= self.config.two_finger_tap_move =>
            {
                TrackpadButton::RightClick
            }
            3 if elapsed_ms <= self.config.three_finger_tap_max_ms
                && session.max_movement() <= self.config.three_finger_tap_move =>
            {
                TrackpadButton::MiddleClick
            }
            _ => return None,
        };

        Some(TrackpadGestureEvent::Click(button))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GestureSession {
    fingers: u8,
    start_ms: u32,
    start_x: i32,
    start_y: i32,
    current_x: i32,
    current_y: i32,
    max_abs_dx: u16,
    max_abs_dy: u16,
    swipe_sent: bool,
}

impl GestureSession {
    fn start(
        fingers: u8,
        frame: CoordinateFrame,
        start_ms: u32,
        axis_transform: TrackpadAxisTransform,
    ) -> Self {
        let (start_x, start_y) = axis_transform.apply(gesture_position(frame, fingers));
        Self {
            fingers,
            start_ms,
            start_x,
            start_y,
            current_x: start_x,
            current_y: start_y,
            max_abs_dx: 0,
            max_abs_dy: 0,
            swipe_sent: false,
        }
    }

    fn update_position(&mut self, frame: CoordinateFrame, axis_transform: TrackpadAxisTransform) {
        let (x, y) = axis_transform.apply(gesture_position(frame, self.fingers));
        self.current_x = x;
        self.current_y = y;

        let dx = abs_delta_u16(x, self.start_x);
        let dy = abs_delta_u16(y, self.start_y);

        if dx > self.max_abs_dx {
            self.max_abs_dx = dx;
        }
        if dy > self.max_abs_dy {
            self.max_abs_dy = dy;
        }
    }

    const fn max_movement(self) -> u16 {
        if self.max_abs_dx > self.max_abs_dy {
            self.max_abs_dx
        } else {
            self.max_abs_dy
        }
    }

    fn swipe_button(self, threshold: u16) -> Option<TrackpadButton> {
        if self.max_abs_dx < threshold && self.max_abs_dy < threshold {
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

fn gesture_position(frame: CoordinateFrame, fingers: u8) -> (i32, i32) {
    if fingers == 2 {
        (
            (i32::from(frame.finger1.x) + i32::from(frame.finger2.x)) / 2,
            (i32::from(frame.finger1.y) + i32::from(frame.finger2.y)) / 2,
        )
    } else {
        (i32::from(frame.finger1.x), i32::from(frame.finger1.y))
    }
}

fn abs_delta_u16(left: i32, right: i32) -> u16 {
    let delta = if left >= right {
        left - right
    } else {
        right - left
    };

    if delta > i32::from(u16::MAX) {
        u16::MAX
    } else {
        delta as u16
    }
}

fn clamp_i32_to_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn clamp_i16_to_i8(value: i16) -> i8 {
    value.clamp(i16::from(i8::MIN), i16::from(i8::MAX)) as i8
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

        assert_eq!(
            config.motion_event(TrackpadSide::Right, 20, -8),
            Some(TrackpadMotionEvent {
                side: TrackpadSide::Right,
                x: 4,
                y: 10,
            })
        );
    }

    fn frame_with_fingers(
        fingers: u8,
        finger1_x: u16,
        finger1_y: u16,
        finger2_x: u16,
        finger2_y: u16,
    ) -> CoordinateFrame {
        CoordinateFrame {
            trackpad_flags: u16::from(fingers),
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
