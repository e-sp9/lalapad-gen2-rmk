use embedded_hal::digital::OutputPin;
use rmk::{
    channel::{CONTROLLER_CHANNEL, ControllerSub},
    controller::{Controller, PollingController},
    event::ControllerEvent,
};

const BATTERY_LEVEL_HIGH: u8 = 30;
const BATTERY_LEVEL_LOW: u8 = 20;
const BATTERY_LEVEL_CRITICAL: u8 = 10;
const BATTERY_BLINK_TICKS: u8 = 20;
const CONNECTION_BLINK_TICKS: u8 = 10;
const LAYER_DEBOUNCE_TICKS: u8 = 1;
const LAYER_BLINK_PHASE_TICKS: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RgbColor {
    Off,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl RgbColor {
    const fn channels(self) -> (bool, bool, bool) {
        match self {
            Self::Off => (false, false, false),
            Self::Red => (true, false, false),
            Self::Green => (false, true, false),
            Self::Yellow => (true, true, false),
            Self::Blue => (false, false, true),
            Self::Magenta => (true, false, true),
            Self::Cyan => (false, true, true),
            Self::White => (true, true, true),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WidgetMode {
    Idle,
    Blink {
        color: RgbColor,
        ticks_left: u8,
    },
    LayerDelay {
        layer: u8,
        ticks_left: u8,
    },
    LayerBlink {
        phases_left: u8,
        phase_ticks_left: u8,
        on: bool,
    },
}

pub struct RgbLedWidget<R: OutputPin, G: OutputPin, B: OutputPin> {
    red: R,
    green: G,
    blue: B,
    low_active: bool,
    sub: ControllerSub,
    mode: WidgetMode,
    current_color: RgbColor,
    battery_seen: bool,
    layer: u8,
    show_layer_change: bool,
}

impl<R: OutputPin, G: OutputPin, B: OutputPin> RgbLedWidget<R, G, B> {
    pub fn new(red: R, green: G, blue: B, low_active: bool, show_layer_change: bool) -> Self {
        let mut widget = Self {
            red,
            green,
            blue,
            low_active,
            sub: CONTROLLER_CHANNEL.subscriber().unwrap(),
            mode: WidgetMode::Idle,
            current_color: RgbColor::Off,
            battery_seen: false,
            layer: 0,
            show_layer_change,
        };
        widget.apply_color(RgbColor::Off);
        widget
    }

    fn queue_blink(&mut self, color: RgbColor, ticks_left: u8) {
        self.mode = WidgetMode::Blink { color, ticks_left };
        self.apply_color(color);
    }

    fn queue_layer_change(&mut self, layer: u8) {
        if layer == 0 {
            self.mode = WidgetMode::Idle;
            self.apply_color(RgbColor::Off);
            return;
        }

        self.mode = WidgetMode::LayerDelay {
            layer,
            ticks_left: LAYER_DEBOUNCE_TICKS,
        };
        self.apply_color(RgbColor::Off);
    }

    fn battery_color(level: u8) -> RgbColor {
        if level >= BATTERY_LEVEL_HIGH {
            RgbColor::Green
        } else if level >= BATTERY_LEVEL_LOW {
            RgbColor::Yellow
        } else {
            RgbColor::Red
        }
    }

    fn apply_color(&mut self, color: RgbColor) {
        if self.current_color == color {
            return;
        }

        let (red, green, blue) = color.channels();
        self.set_red(red);
        self.set_green(green);
        self.set_blue(blue);
        self.current_color = color;
    }

    fn set_red(&mut self, active: bool) {
        set_pin(&mut self.red, self.low_active, active);
    }

    fn set_green(&mut self, active: bool) {
        set_pin(&mut self.green, self.low_active, active);
    }

    fn set_blue(&mut self, active: bool) {
        set_pin(&mut self.blue, self.low_active, active);
    }
}

impl<R: OutputPin, G: OutputPin, B: OutputPin> Controller for RgbLedWidget<R, G, B> {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::Battery(level) => {
                let color = Self::battery_color(level);
                let should_blink = !self.battery_seen || level <= BATTERY_LEVEL_CRITICAL;
                self.battery_seen = true;
                if should_blink {
                    self.queue_blink(color, BATTERY_BLINK_TICKS);
                }
            }
            ControllerEvent::Layer(layer) => {
                if self.show_layer_change && layer != self.layer {
                    self.layer = layer;
                    self.queue_layer_change(layer);
                }
            }
            ControllerEvent::BleState(_, state) => {
                let color = match state {
                    rmk::ble::BleState::Connected => RgbColor::Blue,
                    rmk::ble::BleState::Advertising => RgbColor::Yellow,
                    rmk::ble::BleState::None => RgbColor::Red,
                };
                self.queue_blink(color, CONNECTION_BLINK_TICKS);
            }
            ControllerEvent::SplitPeripheral(_, connected)
            | ControllerEvent::SplitCentral(connected) => {
                self.queue_blink(
                    if connected {
                        RgbColor::Blue
                    } else {
                        RgbColor::Red
                    },
                    CONNECTION_BLINK_TICKS,
                );
            }
            ControllerEvent::Sleep(true) => {
                self.mode = WidgetMode::Idle;
                self.apply_color(RgbColor::Off);
            }
            _ => {}
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

impl<R: OutputPin, G: OutputPin, B: OutputPin> PollingController for RgbLedWidget<R, G, B> {
    const INTERVAL: embassy_time::Duration = embassy_time::Duration::from_millis(100);

    async fn update(&mut self) {
        match self.mode {
            WidgetMode::Idle => {
                self.apply_color(RgbColor::Off);
            }
            WidgetMode::Blink { color, ticks_left } => {
                if ticks_left == 0 {
                    self.mode = WidgetMode::Idle;
                    self.apply_color(RgbColor::Off);
                } else {
                    self.mode = WidgetMode::Blink {
                        color,
                        ticks_left: ticks_left - 1,
                    };
                    self.apply_color(color);
                }
            }
            WidgetMode::LayerDelay { layer, ticks_left } => {
                if ticks_left == 0 {
                    self.mode = WidgetMode::LayerBlink {
                        phases_left: layer.saturating_mul(2),
                        phase_ticks_left: LAYER_BLINK_PHASE_TICKS,
                        on: true,
                    };
                } else {
                    self.mode = WidgetMode::LayerDelay {
                        layer,
                        ticks_left: ticks_left - 1,
                    };
                    self.apply_color(RgbColor::Off);
                }
            }
            WidgetMode::LayerBlink {
                phases_left,
                phase_ticks_left,
                on,
            } => {
                if phases_left == 0 {
                    self.mode = WidgetMode::Idle;
                    self.apply_color(RgbColor::Off);
                } else if phase_ticks_left > 1 {
                    self.mode = WidgetMode::LayerBlink {
                        phases_left,
                        phase_ticks_left: phase_ticks_left - 1,
                        on,
                    };
                    self.apply_color(if on { RgbColor::Cyan } else { RgbColor::Off });
                } else {
                    self.mode = WidgetMode::LayerBlink {
                        phases_left: phases_left - 1,
                        phase_ticks_left: LAYER_BLINK_PHASE_TICKS,
                        on: !on,
                    };
                    self.apply_color(if on { RgbColor::Cyan } else { RgbColor::Off });
                }
            }
        }
    }
}

fn set_pin<P: OutputPin>(pin: &mut P, low_active: bool, active: bool) {
    if active == low_active {
        pin.set_low().ok();
    } else {
        pin.set_high().ok();
    }
}
