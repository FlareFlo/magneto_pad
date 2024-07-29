use defmt::info;
use embassy_futures::join::join3;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber};
use embassy_usb::{Builder, Config, UsbDevice};
use embassy_usb::class::hid::{HidReader, HidReaderWriter, HidWriter};
use shared::{PRODUCT_ID, VENDOR_ID};
use shared::message::Message;
use crate::usb::handlers::{DeviceHandler, RequestHandler};
use crate::usb::hid::HidSetup;
use crate::usb::webusb::WebusbSetup;

mod webusb;
mod hid;
mod gamepad;
mod xinput;
mod handlers;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const MANUFACTURER: &'static str = "magneto_pad_manufacturer";
const PRODUCT: &'static str = "magneto_pad_product";
const SERIAL_NUMBER: &'static str = "magneto_pad_serial_nr";

pub struct UsbSetup<'d> {
    pub hid_setup: HidSetup<'d>,
    pub webusb_setup: WebusbSetup<'d>,
    usb: UsbDevice<'d, Driver<'d, USB>>
}

impl UsbSetup<'_> {
    pub fn new(usb: USB) -> Self {
        // Create the driver, from the HAL.
        let driver = Driver::new(usb, Irqs);

        // Create embassy-usb Config
        let mut config = Config::new(VENDOR_ID, PRODUCT_ID);
        config.manufacturer = Some(MANUFACTURER);
        config.product = Some(PRODUCT);
        config.serial_number = Some(SERIAL_NUMBER);
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config.composite_with_iads = true;
        config.device_class = 0xEF;
        config.device_protocol = 0x01;
        config.device_sub_class = 0x02;

        let mut config_descriptor = [0; 256];
        let mut bos_descriptor = [0; 256];
        let mut msos_descriptor = [0; 256];
        let mut control_buf = [0; 64];

        let mut device_handler = DeviceHandler::new();

        let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
        );

        builder.handler(&mut device_handler);

        let hid_setup = HidSetup::new(&mut builder);

        let webusb_setup = WebusbSetup::new(&mut builder);

        let usb = builder.build();

        Self {
            hid_setup,
            webusb_setup,
            usb,
        }
    }

    pub async fn run(&mut self) {
        let mut request_handler = RequestHandler { };
        join3(
            self.usb.run(),
            self.hid_setup.run_reader(&mut request_handler),
            self.webusb_setup.run(),
        ).await;
    }
}
