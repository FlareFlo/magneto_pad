#![no_std]
#![no_main]

mod usb;

use core::sync::atomic::{AtomicBool, Ordering};

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input,Pull};
use embassy_rp::peripherals::{USB};
use embassy_usb::class::hid::{HidReaderWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config, Handler};
use embassy_usb::class::web_usb::{Config as WebUsbConfig, State as WebUsbState, WebUsb};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber, WaitResult};
use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use {defmt_rtt as _, panic_probe as _};
use shared::{PRODUCT_ID, VENDOR_ID};
use shared::message::{ Message};
use crate::usb::UsbSetup;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut usb = UsbSetup::new(p.USB);

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_24, Pull::Up);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    let mut writer = usb.hid_setup.writer;

    // Do stuff with the class!
    let in_fut = async {
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
            // Send the report.
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
            signal_pin.wait_for_low().await;
            info!("LOW DETECTED");
            let report = KeyboardReport {
                keycodes: [4, 0, 0, 0, 0, 0],
                leds: 0,
                modifier: 0,
                reserved: 0,
            };
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
        }
    };

    let ping_pong  = async {
        let mut sub = usb.webusb_setup.channel.subscriber().unwrap();
        let publisher = usb.webusb_setup.channel.publisher().unwrap();
        loop {
            match sub.next_message().await {
                WaitResult::Lagged(x) => error!("Channel lagged for {} messages", x),
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

    join3(usb.run(), in_fut, ping_pong).await;
}
