#![no_std]
#![no_main]

mod usb;
mod hid;


use embassy_executor::Spawner;
use embassy_futures::join::join;
use usbd_hid::descriptor::KeyboardReport;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use {defmt_rtt as _, panic_probe as _};
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