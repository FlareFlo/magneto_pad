use defmt::{error, info, warn};
use embassy_futures::join::join;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber, WaitResult};
use embassy_usb::Builder;
use embassy_usb::class::web_usb::{Config, State, WebUsb};
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
use shared::message::Message;

type UsbChannel = PubSubChannel<NoopRawMutex, (bool, Message), 10, 2, 2>;
type UsbSubscriber<'a> = Subscriber<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;
type UsbPublisher<'a> = Publisher<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;

struct WebusbEndpoints<'d> {
    write_ep: <embassy_rp::usb::Driver<'d, USB> as embassy_usb::driver::Driver<'d>>::EndpointIn,
    read_ep: <embassy_rp::usb::Driver<'d, USB> as embassy_usb::driver::Driver<'d>>::EndpointOut,
    config: Config<'d>,
    state: State<'d>,
}

impl<'d> WebusbEndpoints<'d> {
    fn new(builder: &mut Builder<'d, Driver<'d, USB>>) -> Self {
        let mut state = State::new();
        let config = Config {
            max_packet_size,
            landing_url: None,
            vendor_code: 1,
        };

        WebUsb::configure(builder, &mut state, &config);

        let mut func = builder.function(0xff, 0x00, 0x00);
        let mut iface = func.interface();
        let mut alt = iface.alt_setting(0xff, 0x00, 0x00, None);

        let write_ep = alt.endpoint_bulk_in(config.max_packet_size);
        let read_ep = alt.endpoint_bulk_out(config.max_packet_size);
        warn!("{} {}",read_ep.info().addr, write_ep.info().addr);

        WebusbEndpoints { write_ep, read_ep, config, state }
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
                    WaitResult::Lagged(x) => error!("Channel lagged for {} messages", x),
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

pub struct WebusbSetup<'d> {
    webusb_endpoints: WebusbEndpoints<'d>,
    pub channel: UsbChannel,
}

impl<'d> WebusbSetup<'d> {
    pub fn new(builder: &mut Builder<'d, Driver<'d, USB>>) -> WebusbSetup<'d> {
        Self {
            channel: PubSubChannel::new(),
            webusb_endpoints: WebusbEndpoints::new(builder),
        }
    }

    pub async fn run(&mut self) {
        loop {
            self.webusb_endpoints.wait_connected().await;
            info!("Connected webusb");
            self.webusb_endpoints.run_webusb(self.channel.publisher().unwrap(), self.channel.subscriber().unwrap()).await;
        }
    }
}