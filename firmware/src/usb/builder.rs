use embassy_stm32::peripherals::{PA11, PA12, USB_OTG_FS};
use embassy_stm32::usb_otg::Driver;
use embassy_usb::Builder;
use crate::{make_static};
use crate::usb::config::get_usb_config;
use crate::usb::Irqs;

struct Buffers {
	config_descriptor : [u8; 256],
	bos_descriptor : [u8; 256],
	msos_descriptor : [u8; 256],
	control_buf : [u8; 64],
}


pub fn get_builder(usb: USB_OTG_FS, pa12: PA12, pa11: PA11) -> Builder<'static, Driver<'static, USB_OTG_FS>>{
	// Create the driver, from the HAL.

	let mut ep_out_buffer = make_static!([u8; 256], [0u8; 256]);
	let mut config = embassy_stm32::usb_otg::Config::default();
	let driver = Driver::new_fs(usb, Irqs, pa12, pa11, ep_out_buffer, config);

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