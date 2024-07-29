#![no_std]
#![no_main]

mod usb;
mod hid;

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
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber, WaitResult};
use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use {defmt_rtt as _, panic_probe as _};
use shared::{PRODUCT_ID, VENDOR_ID};
use shared::message::{ Message};
use crate::hid::run_hid;
use crate::usb::setup_usb;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let channel: Channel<NoopRawMutex, KeyboardReport, 10> = Channel::new();
    let hid = run_hid(p.PIN_24, channel.sender());
    let usb = setup_usb(p.USB, channel.receiver());

    join(hid, usb).await;
}