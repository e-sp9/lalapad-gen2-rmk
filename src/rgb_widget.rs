use embedded_hal::digital::OutputPin;
#[cfg(target_arch = "arm")]
use rmk::{
    channel::{CONTROLLER_CHANNEL, ControllerSub},
    controller::{Controller, PollingController},
    event::ControllerEvent,
};

#[cfg(not(target_arch = "arm"))]
use crate::host_test_stubs::rmk::{
    self,
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

    fn set_idle(&mut self) {
        self.mode = WidgetMode::Idle;
        self.apply_color(RgbColor::Off);
    }

    fn queue_blink(&mut self, color: RgbColor, ticks_left: u8) {
        self.mode = WidgetMode::Blink { color, ticks_left };
        self.apply_color(color);
    }

    fn queue_layer_change(&mut self, layer: u8) {
        if layer == 0 {
            self.set_idle();
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
                self.set_idle();
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
                    self.set_idle();
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
                    self.set_idle();
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::{
        convert::Infallible,
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PinLevel {
        Low,
        High,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MockPin {
        level: PinLevel,
    }

    struct PendingOnce {
        pending: bool,
    }

    impl Future for PendingOnce {
        type Output = u8;

        fn poll(mut self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.pending {
                self.pending = false;
                Poll::Pending
            } else {
                Poll::Ready(7)
            }
        }
    }

    impl MockPin {
        const fn new() -> Self {
            Self {
                level: PinLevel::High,
            }
        }
    }

    impl embedded_hal::digital::ErrorType for MockPin {
        type Error = Infallible;
    }

    impl OutputPin for MockPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.level = PinLevel::Low;
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.level = PinLevel::High;
            Ok(())
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);
        let mut future = pin!(future);
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
        }
    }

    #[test]
    fn block_on_polls_again_after_pending() {
        assert_eq!(block_on(PendingOnce { pending: true }), 7);
    }

    #[test]
    fn battery_color_matches_upstream_thresholds() {
        assert_eq!(
            RgbLedWidget::<MockPin, MockPin, MockPin>::battery_color(30),
            RgbColor::Green
        );
        assert_eq!(
            RgbLedWidget::<MockPin, MockPin, MockPin>::battery_color(20),
            RgbColor::Yellow
        );
        assert_eq!(
            RgbLedWidget::<MockPin, MockPin, MockPin>::battery_color(10),
            RgbColor::Red
        );
    }

    #[test]
    fn active_low_color_drives_xiao_rgb_pins_low_when_on() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        widget.apply_color(RgbColor::Cyan);

        assert_eq!(widget.red.level, PinLevel::High);
        assert_eq!(widget.green.level, PinLevel::Low);
        assert_eq!(widget.blue.level, PinLevel::Low);
    }

    #[test]
    fn active_high_color_drives_channels_high_when_on() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), false, true);

        widget.apply_color(RgbColor::Magenta);

        assert_eq!(widget.red.level, PinLevel::High);
        assert_eq!(widget.green.level, PinLevel::Low);
        assert_eq!(widget.blue.level, PinLevel::High);
    }

    #[test]
    fn color_channel_table_covers_all_status_colors() {
        assert_eq!(RgbColor::Off.channels(), (false, false, false));
        assert_eq!(RgbColor::Red.channels(), (true, false, false));
        assert_eq!(RgbColor::Green.channels(), (false, true, false));
        assert_eq!(RgbColor::Yellow.channels(), (true, true, false));
        assert_eq!(RgbColor::Blue.channels(), (false, false, true));
        assert_eq!(RgbColor::Magenta.channels(), (true, false, true));
        assert_eq!(RgbColor::Cyan.channels(), (false, true, true));
        assert_eq!(RgbColor::White.channels(), (true, true, true));
    }

    #[test]
    fn battery_event_blinks_then_returns_to_idle() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        block_on(widget.process_event(ControllerEvent::Battery(30)));

        assert_eq!(widget.current_color, RgbColor::Green);
        assert_eq!(
            widget.mode,
            WidgetMode::Blink {
                color: RgbColor::Green,
                ticks_left: BATTERY_BLINK_TICKS
            }
        );

        for _ in 0..=BATTERY_BLINK_TICKS {
            block_on(widget.update());
        }

        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.current_color, RgbColor::Off);
    }

    #[test]
    fn critical_battery_requeues_after_first_battery_report() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        block_on(widget.process_event(ControllerEvent::Battery(30)));
        block_on(widget.process_event(ControllerEvent::Battery(25)));
        assert_eq!(widget.current_color, RgbColor::Green);

        block_on(widget.process_event(ControllerEvent::Battery(10)));

        assert_eq!(widget.current_color, RgbColor::Red);
        assert_eq!(
            widget.mode,
            WidgetMode::Blink {
                color: RgbColor::Red,
                ticks_left: BATTERY_BLINK_TICKS
            }
        );
    }

    #[test]
    fn layer_change_debounces_then_blinks_cyan() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        block_on(widget.process_event(ControllerEvent::Layer(2)));
        assert_eq!(
            widget.mode,
            WidgetMode::LayerDelay {
                layer: 2,
                ticks_left: LAYER_DEBOUNCE_TICKS
            }
        );

        block_on(widget.update());
        assert_eq!(
            widget.mode,
            WidgetMode::LayerDelay {
                layer: 2,
                ticks_left: 0
            }
        );

        block_on(widget.update());
        assert_eq!(
            widget.mode,
            WidgetMode::LayerBlink {
                phases_left: 4,
                phase_ticks_left: LAYER_BLINK_PHASE_TICKS,
                on: true,
            }
        );

        block_on(widget.update());
        assert_eq!(widget.current_color, RgbColor::Cyan);

        block_on(widget.process_event(ControllerEvent::Layer(0)));
        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.current_color, RgbColor::Off);
    }

    #[test]
    fn connection_and_sleep_events_map_to_status_colors() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        block_on(widget.process_event(ControllerEvent::BleState(0, rmk::ble::BleState::Connected)));
        assert_eq!(widget.current_color, RgbColor::Blue);

        block_on(widget.process_event(ControllerEvent::BleState(
            0,
            rmk::ble::BleState::Advertising,
        )));
        assert_eq!(widget.current_color, RgbColor::Yellow);

        block_on(widget.process_event(ControllerEvent::BleState(0, rmk::ble::BleState::None)));
        assert_eq!(widget.current_color, RgbColor::Red);

        block_on(widget.process_event(ControllerEvent::SplitPeripheral(0, true)));
        assert_eq!(widget.current_color, RgbColor::Blue);

        block_on(widget.process_event(ControllerEvent::SplitCentral(false)));
        assert_eq!(widget.current_color, RgbColor::Red);

        block_on(widget.process_event(ControllerEvent::Sleep(true)));
        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.current_color, RgbColor::Off);
    }

    #[test]
    fn ignored_events_and_idle_layer_edges_do_not_change_status() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, false);

        block_on(widget.update());
        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.current_color, RgbColor::Off);

        block_on(widget.process_event(ControllerEvent::Other));
        block_on(widget.process_event(ControllerEvent::Sleep(false)));
        block_on(widget.process_event(ControllerEvent::Layer(1)));
        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.layer, 0);

        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);
        block_on(widget.process_event(ControllerEvent::Layer(2)));
        block_on(widget.process_event(ControllerEvent::Layer(2)));
        assert_eq!(
            widget.mode,
            WidgetMode::LayerDelay {
                layer: 2,
                ticks_left: LAYER_DEBOUNCE_TICKS
            }
        );
    }

    #[test]
    fn layer_blink_handles_long_phase_and_done_phase() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);

        widget.mode = WidgetMode::LayerBlink {
            phases_left: 2,
            phase_ticks_left: 2,
            on: true,
        };
        block_on(widget.update());
        assert_eq!(
            widget.mode,
            WidgetMode::LayerBlink {
                phases_left: 2,
                phase_ticks_left: 1,
                on: true,
            }
        );
        assert_eq!(widget.current_color, RgbColor::Cyan);

        widget.mode = WidgetMode::LayerBlink {
            phases_left: 2,
            phase_ticks_left: 2,
            on: false,
        };
        block_on(widget.update());
        assert_eq!(
            widget.mode,
            WidgetMode::LayerBlink {
                phases_left: 2,
                phase_ticks_left: 1,
                on: false,
            }
        );
        assert_eq!(widget.current_color, RgbColor::Off);

        widget.mode = WidgetMode::LayerBlink {
            phases_left: 2,
            phase_ticks_left: 1,
            on: false,
        };
        block_on(widget.update());
        assert_eq!(
            widget.mode,
            WidgetMode::LayerBlink {
                phases_left: 1,
                phase_ticks_left: LAYER_BLINK_PHASE_TICKS,
                on: true,
            }
        );
        assert_eq!(widget.current_color, RgbColor::Off);

        widget.mode = WidgetMode::LayerBlink {
            phases_left: 0,
            phase_ticks_left: 1,
            on: false,
        };
        block_on(widget.update());
        assert_eq!(widget.mode, WidgetMode::Idle);
        assert_eq!(widget.current_color, RgbColor::Off);
    }

    #[test]
    #[should_panic(expected = "host test controller subscriber is not implemented")]
    fn next_message_uses_controller_subscription() {
        let mut widget =
            RgbLedWidget::new(MockPin::new(), MockPin::new(), MockPin::new(), true, true);
        let _ = block_on(widget.next_message());
    }
}
