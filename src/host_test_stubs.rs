pub(crate) mod rmk {
    pub mod ble {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum BleState {
            Connected,
            Advertising,
            None,
        }
    }

    pub mod event {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct KeyboardEvent {
            pub row: u8,
            pub col: u8,
            pub pressed: bool,
        }

        impl KeyboardEvent {
            pub const fn key(row: u8, col: u8, pressed: bool) -> Self {
                Self { row, col, pressed }
            }
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ControllerEvent {
            Battery(u8),
            Layer(u8),
            BleState(u8, super::ble::BleState),
            SplitPeripheral(usize, bool),
            SplitCentral(bool),
            Sleep(bool),
            Other,
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum Event {
            Key(KeyboardEvent),
            Custom([u8; 16]),
        }
    }

    pub mod hid {
        use usbd_hid::descriptor::MouseReport;

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum Report {
            MouseReport(MouseReport),
        }
    }

    pub mod state {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ConnectionType {
            Usb,
            Ble,
        }

        pub fn get_active_connection_type() -> ConnectionType {
            ConnectionType::Usb
        }
    }

    pub mod channel {
        use super::{event::Event, event::KeyboardEvent, hid::Report};

        pub struct Channel<T> {
            _marker: core::marker::PhantomData<T>,
        }

        impl<T> Channel<T> {
            pub const fn new() -> Self {
                Self {
                    _marker: core::marker::PhantomData,
                }
            }

            pub async fn send(&self, _value: T) {}

            pub async fn receive(&self) -> T {
                panic!("host test channel receive is not implemented")
            }

            pub fn try_send(&self, _value: T) -> Result<(), ()> {
                Ok(())
            }

            pub fn try_receive(&self) -> Result<T, ()> {
                Err(())
            }

            pub fn is_full(&self) -> bool {
                false
            }
        }

        pub struct ControllerSub;

        impl ControllerSub {
            pub async fn next_message_pure(&mut self) -> super::event::ControllerEvent {
                panic!("host test controller subscriber is not implemented")
            }
        }

        pub struct ControllerChannel;

        impl ControllerChannel {
            pub const fn new() -> Self {
                Self
            }

            pub fn subscriber(&self) -> Result<ControllerSub, ()> {
                Ok(ControllerSub)
            }
        }

        pub static KEY_EVENT_CHANNEL: Channel<KeyboardEvent> = Channel::new();
        pub static EVENT_CHANNEL: Channel<Event> = Channel::new();
        pub static KEYBOARD_REPORT_CHANNEL: Channel<Report> = Channel::new();
        pub static CONTROLLER_CHANNEL: ControllerChannel = ControllerChannel::new();
    }

    pub mod controller {
        pub trait Controller {
            type Event;

            async fn process_event(&mut self, event: Self::Event);
            async fn next_message(&mut self) -> Self::Event;
        }

        pub trait PollingController: Controller {
            const INTERVAL: embassy_time::Duration;

            async fn update(&mut self);
        }
    }

    pub mod input_device {
        pub trait InputDevice {
            async fn read_event(&mut self) -> super::event::Event;
        }
    }
}
