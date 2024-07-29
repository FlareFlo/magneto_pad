use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_usb::Builder;
use embassy_usb::class::hid::{Config, HidReader, HidReaderWriter, HidWriter, RequestHandler, State};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

pub struct HidSetup<'d> {
    state: State<'d>,
    reader: Option<HidReader<'d, Driver<'d, USB>, 1>>,
    pub writer: HidWriter<'d, Driver<'d, USB>, 8>,
}

impl<'d> HidSetup<'d> {
    pub fn new(builder: &mut Builder<'d, Driver<'d, USB>>) -> HidSetup<'d> {
        let mut state = State::new();
        let config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 60,
            max_packet_size: 64,
        };

        let reader_writer = HidReaderWriter::<_, 1, 8>::new(builder, &mut state, config);
        let (reader, writer) = reader_writer.split();

        Self {
            reader: Some(reader),
            writer,
            state,
        }
    }

    pub async fn run_reader<T: RequestHandler>(&mut self, request_handler: &mut T) {
        if let Some(reader) = self.reader.take() {
            reader.run(false, request_handler).await;
        }
    }
}
