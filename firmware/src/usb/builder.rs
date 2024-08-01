use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_usb::Builder;
use crate::make_static;
use crate::usb::config::get_usb_config;
use crate::usb::{Irqs};
use crate::usb::UsbDriver;

struct Buffers {
	config_descriptor : [u8; 256],
	bos_descriptor : [u8; 256],
	msos_descriptor : [u8; 256],
	control_buf : [u8; 64],
}


pub fn get_builder(usb: USB) -> Builder<'static, Driver<'static, USB>>{
	// Create the driver, from the HAL.
	let driver = UsbDriver::new(usb, Irqs);

	let config = get_usb_config();

	let bufs = make_static!(Buffers, Buffers {
config_descriptor: [0; 256],bos_descriptor: [0; 256],msos_descriptor: [0; 256],control_buf: [0; 64],});


	Builder::new(
		driver,
		config,
		&mut bufs.config_descriptor,
		&mut bufs.bos_descriptor,
		&mut bufs.msos_descriptor,
		&mut bufs.control_buf,
	)
}