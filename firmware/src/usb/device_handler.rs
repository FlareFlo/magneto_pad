use core::sync::atomic::{AtomicBool, Ordering};
use defmt::info;
use embassy_usb::Handler;
use crate::make_static;

pub struct DeviceHandler {
	configured: AtomicBool,
}

impl DeviceHandler {
	pub(crate) fn new() -> &'static mut Self {
		make_static!(DeviceHandler, DeviceHandler {
			configured: AtomicBool::new(false),
		})
	}
}

impl Handler for DeviceHandler {
	fn enabled(&mut self, enabled: bool) {
		self.configured.store(false, Ordering::Relaxed);
		if enabled {
			info!("Device enabled");
		} else {
			info!("Device disabled");
		}
	}

	fn reset(&mut self) {
		self.configured.store(false, Ordering::Relaxed);
		info!("Bus reset, the Vbus current limit is 100mA");
	}

	fn addressed(&mut self, addr: u8) {
		self.configured.store(false, Ordering::Relaxed);
		info!("USB address set to: {}", addr);
	}

	fn configured(&mut self, configured: bool) {
		self.configured.store(configured, Ordering::Relaxed);
		if configured {
			info!("Device configured, it may now draw up to the configured current limit from Vbus.")
		} else {
			info!("Device is no longer configured, the Vbus current limit is 100mA.");
		}
	}
}