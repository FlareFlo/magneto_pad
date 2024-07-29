use core::sync::atomic::{AtomicBool, Ordering};

use defmt::*;
use embassy_futures::join::{join, join3};
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_usb::class::hid::{HidReaderWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config, Handler};
use embassy_usb::class::web_usb::{Config as WebUsbConfig, State as WebUsbState, WebUsb};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber, WaitResult};
use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use {defmt_rtt as _, panic_probe as _};
use shared::{PRODUCT_ID, VENDOR_ID};
use shared::message::{ Message};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type UsbChannel = PubSubChannel<NoopRawMutex, (bool, Message), 10, 2, 2>;
type UsbSubscriber<'a> = Subscriber<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;
type UsbPublisher<'a> = Publisher<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;

pub async fn setup_usb(usb: USB, receiver: Receiver<'_, NoopRawMutex, KeyboardReport, 10>) {
	// Create the driver, from the HAL.
	let driver = UsbDriver::new(usb, Irqs);

	// Create embassy-usb Config
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

	// Create embassy-usb DeviceBuilder using the driver and config.
	// It needs some buffers for building the descriptors.
	let mut config_descriptor = [0; 256];
	let mut bos_descriptor = [0; 256];
	// You can also add a Microsoft OS descriptor.
	let mut msos_descriptor = [0; 256];
	let mut control_buf = [0; 64];
	let mut request_handler = MyRequestHandler {};
	let mut device_handler = MyDeviceHandler::new();

	let mut state = State::new();
	let mut web_state = WebUsbState::new();

	let web_usb_config = WebUsbConfig {
		max_packet_size: 64,
		landing_url: None,
		vendor_code: 1,
	};


	let mut builder = Builder::new(
		driver,
		config,
		&mut config_descriptor,
		&mut bos_descriptor,
		&mut msos_descriptor,
		&mut control_buf,
	);

	builder.handler(&mut device_handler);

	// Create classes on the builder.
	let config = embassy_usb::class::hid::Config {
		report_descriptor: KeyboardReport::desc(),
		request_handler: None,
		poll_ms: 60,
		max_packet_size: 64,
	};

	let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, &mut state, config);
	WebUsb::configure(&mut builder, &mut web_state, &web_usb_config);

	let mut endpoints = WebEndpoints::new(&mut builder, &web_usb_config);

	// Build the builder.
	let mut usb = builder.build();

	// Run the USB device.
	let usb_fut = usb.run();

	let (reader, mut writer) = hid.split();

	let hid_writer_fut = async {
		loop {
			let msg = receiver.receive().await;
			writer.write_serialize(&msg).await.unwrap();
		}
	};

	let out_fut = async {
		reader.run(false, &mut request_handler).await;
	};

	let channel: UsbChannel = PubSubChannel::new();

	let webusb = async {
		loop {
			endpoints.wait_connected().await;
			info!("Connected webusb");
			endpoints.run_webusb(channel.publisher().unwrap(), channel.subscriber().unwrap()).await;
		}
	};

	let ping_pong  = async {
		let mut sub = channel.subscriber().unwrap();
		let publisher = channel.publisher().unwrap();
		loop {
			match sub.next_message().await {
				WaitResult::Lagged(x) => {error!("Channel lagged for {} messages", x)}
				WaitResult::Message((should_write, msg)) => {
					if !should_write {
						match msg {
							Message::Ping => {
								publisher.publish((true, Message::Pong)).await;
							}
							_ => {}
						}
					}
				}
			}
		}
	};

	join3(join(usb_fut, webusb), join(hid_writer_fut, out_fut), ping_pong).await;
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
	fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
		info!("Get report for {:?}", id);
		None
	}

	fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
		info!("Set report for {:?}: {=[u8]}", id, data);
		OutResponse::Accepted
	}

	fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
		info!("Set idle rate for {:?} to {:?}", id, dur);
	}

	fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
		info!("Get idle rate for {:?}", id);
		None
	}
}

struct MyDeviceHandler {
	configured: AtomicBool,
}

impl MyDeviceHandler {
	fn new() -> Self {
		MyDeviceHandler {
			configured: AtomicBool::new(false),
		}
	}
}

impl Handler for MyDeviceHandler {
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

struct WebEndpoints<'d, D: Driver<'d>> {
	write_ep: D::EndpointIn,
	read_ep: D::EndpointOut,
}

impl<'d, D: Driver<'d>> WebEndpoints<'d, D> {
	fn new(builder: &mut Builder<'d, D>, config: &'d WebUsbConfig<'d>) -> Self {
		let mut func = builder.function(0xff, 0x00, 0x00);
		let mut iface = func.interface();
		let mut alt = iface.alt_setting(0xff, 0x00, 0x00, None);

		let write_ep = alt.endpoint_bulk_in(config.max_packet_size);
		let read_ep = alt.endpoint_bulk_out(config.max_packet_size);
		warn!("{} {}",read_ep.info().addr, write_ep.info().addr);

		WebEndpoints { write_ep, read_ep }
	}

	// Wait until the device's endpoints are enabled.
	async fn wait_connected(&mut self) {
		self.read_ep.wait_enabled().await
	}

	async fn run_webusb(&mut self, publisher: UsbPublisher<'_>, mut sub: UsbSubscriber<'_>) {
		let reader = async {
			let mut buf = [0; 64];
			loop {
				let n = self.read_ep.read(&mut buf).await.unwrap();
				let data = &buf[..n];

				let msg = Message::deserialize(data);
				publisher.publish((false, msg)).await;
			}
		};
		let writer = async {
			loop {
				match sub.next_message().await {
					WaitResult::Lagged(x) => {error!("Channel lagged for {} messages", x)}
					WaitResult::Message((should_write, msg)) => {
						if should_write {
							let ser = msg.serialize();
							self.write_ep.write(ser.as_slice()).await.unwrap();
						}
					}
				}
			}
		};
		join(reader, writer).await;
	}
}