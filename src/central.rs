#![no_main]
#![no_std]

use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {
    use lalapad_gen2_rmk::iqs9151::{
        Iqs9151InputDevice, Iqs9151KeyboardController, Iqs9151MotionOutput, Iqs9151ReadyPin,
        Iqs9151SplitEventController, TrackpadLayerController, TrackpadSide,
    };
    use lalapad_gen2_rmk::rgb_widget::RgbLedWidget;
    use rmk::controller::PollingController;
    use static_cell::StaticCell;

    #[controller(event)]
    fn split_trackpad_events() {
        Iqs9151SplitEventController::new()
    }

    #[controller(event)]
    fn trackpad_layer_state() {
        TrackpadLayerController::new()
    }

    #[controller(poll)]
    fn rgb_led_widget() {
        let red = ::embassy_nrf::gpio::Output::new(
            p.P1_03,
            ::embassy_nrf::gpio::Level::High,
            ::embassy_nrf::gpio::OutputDrive::Standard,
        );
        let green = ::embassy_nrf::gpio::Output::new(
            p.P1_05,
            ::embassy_nrf::gpio::Level::High,
            ::embassy_nrf::gpio::OutputDrive::Standard,
        );
        let blue = ::embassy_nrf::gpio::Output::new(
            p.P1_07,
            ::embassy_nrf::gpio::Level::High,
            ::embassy_nrf::gpio::OutputDrive::Standard,
        );

        RgbLedWidget::new(red, green, blue, true, true)
    }

    #[controller(event)]
    fn right_trackpad() {
        ::embassy_nrf::bind_interrupts!(struct Iqs9151Irqs {
            TWISPI0 => ::embassy_nrf::twim::InterruptHandler<::embassy_nrf::peripherals::TWISPI0>;
        });

        static IQS9151_TX_BUFFER: StaticCell<[u8; 2]> = StaticCell::new();
        let tx_buffer = &mut IQS9151_TX_BUFFER.init([0; 2])[..];
        let mut i2c_config = ::embassy_nrf::twim::Config::default();
        i2c_config.frequency = ::embassy_nrf::twim::Frequency::K400;
        i2c_config.sda_pullup = true;
        i2c_config.scl_pullup = true;
        let i2c = ::embassy_nrf::twim::Twim::new(
            p.TWISPI0,
            Iqs9151Irqs,
            p.P0_04,
            p.P0_05,
            i2c_config,
            tx_buffer,
        );
        let ready = Iqs9151ReadyPin::active_low(::embassy_nrf::gpio::Input::new(
            p.P1_11,
            ::embassy_nrf::gpio::Pull::Up,
        ));
        let mut device = Iqs9151InputDevice::with_ready_pin(i2c, ready, TrackpadSide::Right);
        device.set_motion_output(Iqs9151MotionOutput::HidReport);

        Iqs9151KeyboardController::new_central(device)
    }
}
