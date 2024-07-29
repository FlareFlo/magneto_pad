use defmt::{info, warn};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::PIN_24;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;
use usbd_hid::descriptor::KeyboardReport;

pub async fn run_hid(pin: PIN_24, channel: Sender<'_, NoopRawMutex, KeyboardReport, 10>) {
	// Set up the signal pin that will be used to trigger the keyboard.
	let mut signal_pin = Input::new(pin, Pull::Up);

	// Enable the schmitt trigger to slightly debounce.
	signal_pin.set_schmitt(true);
	loop {
		info!("Waiting for HIGH on pin 16");
		signal_pin.wait_for_high().await;
		info!("HIGH DETECTED");
		// Create a report with the A key pressed. (no shift modifier)
		let report = KeyboardReport {
			keycodes: [0, 0, 0, 0, 0, 0],
			leds: 0,
			modifier: 0,
			reserved: 0,
		};
		channel.send(report).await;
		signal_pin.wait_for_low().await;
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