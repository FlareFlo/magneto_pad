mod config;
mod builder;
mod web_usb;
mod device_handler;

use defmt::*;
use embassy_futures::join::{join, join3};
use embassy_stm32::{bind_interrupts, peripherals, usb};
use embassy_stm32::peripherals::{PA11, PA12, USB_OTG_FS};
use embassy_usb::class::hid::{HidReaderWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder};
use embassy_usb::class::web_usb::{Config as WebUsbConfig, State as WebUsbState, WebUsb};
use usbd_hid::descriptor::{KeyboardReport};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_sync::pubsub::{PubSubChannel, WaitResult};
use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use {defmt_rtt as _, panic_probe as _};
use shared::message::{ Message};
use crate::{make_static};
use crate::usb::builder::get_builder;
use crate::usb::config::{get_device_configs};
use crate::usb::device_handler::DeviceHandler;
use crate::usb::web_usb::{UsbChannel, UsbPublisher, UsbSubscriber};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

pub fn get_states() -> &'static mut (State<'static>, WebUsbState<'static>) {
	let state = State::new();
	let web_state = WebUsbState::new();
	make_static!((State, WebUsbState), (state, web_state))
}

pub async fn setup_usb(usb: USB_OTG_FS, receiver: Receiver<'_, NoopRawMutex, KeyboardReport, 10>, pa12: PA12, pa11: PA11) {
	let (state, web_state) = get_states();

	let device_handler = DeviceHandler::new();
	let mut request_handler = MyRequestHandler {};

	let mut builder = get_builder(usb, pa12, pa11);
	builder.handler(device_handler);

	let (config, web_usb_config) = get_device_configs();

	let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config);
	WebUsb::configure(&mut builder, web_state, &web_usb_config);

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