use embassy_usb::class::hid;
use embassy_usb::Config;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use shared::{PRODUCT_ID, VENDOR_ID};
use embassy_usb::class::web_usb::{Config as WebUsbConfig};

pub fn get_usb_config() -> Config<'static> {
	let mut config = Config::new(VENDOR_ID, PRODUCT_ID);
	config.manufacturer = Some("magneto_pad_manufacturer");
	config.product = Some("magneto_pad_product");
	config.serial_number = Some("magneto_pad_serial_nr");
	config.max_power = 100;
	config.max_packet_size_0 = 64;
	config.composite_with_iads = true;
	config.device_class = 0xEF;
	config.device_protocol = 0x01;
	config.device_sub_class = 0x02;
	config
}

pub fn get_device_configs() -> (hid::Config<'static>, &'static WebUsbConfig<'static>) {
	static WEB_USB_CONFIG: WebUsbConfig = WebUsbConfig {
		max_packet_size: 64,
		landing_url: None,
		vendor_code: 1,
	};
	let config = hid::Config {
		report_descriptor: KeyboardReport::desc(),
		request_handler: None,
		poll_ms: 60,
		max_packet_size: 64,
	};
	(config, &WEB_USB_CONFIG)
}