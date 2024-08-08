use defmt::info;
use embassy_stm32::exti::{AnyChannel, ExtiInput};
use embassy_stm32::gpio::{AnyPin, Input, Pull};
use embassy_stm32::peripherals::{EXTI0};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Sender};
use usbd_hid::descriptor::KeyboardReport;

pub type HidChannel = Channel<NoopRawMutex, KeyboardReport, 10>;

pub async fn run_hid(pin: AnyPin, exti: AnyChannel, channel: Sender<'_, NoopRawMutex, KeyboardReport, 10>) {
	// Set up the signal pin that will be used to trigger the keyboard.
	let mut signal_pin = ExtiInput::new(Input::new(pin, Pull::Down), exti);

	loop {
		info!("Waiting for HIGH on pin 16");
		signal_pin.wait_for_rising_edge().await;
		info!("HIGH DETECTED");
		// Create a report with the A key pressed. (no shift modifier)
		let report = KeyboardReport {
			keycodes: [0, 0, 0, 0, 0, 0],
			leds: 0,
			modifier: 0,
			reserved: 0,
		};
		channel.send(report).await;
		signal_pin.wait_for_falling_edge().await;
		info!("LOW DETECTED");
		let report = KeyboardReport {
			keycodes: [4, 0, 0, 0, 0, 0],
			leds: 0,
			modifier: 0,
			reserved: 0,
		};
		channel.send(report).await;
	}
}