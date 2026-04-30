#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    use lalapad_gen2_rmk::iqs9151::{Iqs9151InputDevice, Iqs9151KeyboardController, TrackpadSide};
    use static_cell::StaticCell;

    #[controller(event)]
    fn left_trackpad() {
        ::embassy_nrf::bind_interrupts!(struct Iqs9151Irqs {
            TWISPI0 => ::embassy_nrf::twim::InterruptHandler<::embassy_nrf::peripherals::TWISPI0>;
        });

        static IQS9151_TX_BUFFER: StaticCell<[u8; 2]> = StaticCell::new();
        let tx_buffer = &mut IQS9151_TX_BUFFER.init([0; 2])[..];
        let mut i2c_config = ::embassy_nrf::twim::Config::default();
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
        let device = Iqs9151InputDevice::new(i2c, TrackpadSide::Left);

        Iqs9151KeyboardController::new(device)
    }
}
