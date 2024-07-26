mod kb_handle;

use std::time::Duration;
use rusb::{Device, GlobalContext};
use shared::message::Message;
use shared::VENDOR_ID;

fn main() {
    let device = get_keyboard().unwrap();
    println!("Found keyboard! Bus {:03} Device {:03}", device.bus_number(), device.address());

    let kb = device.open().unwrap();
    kb.claim_interface(2).unwrap();
    kb.write_bulk(2, Message::toggle_led().serialize().as_slice(), Duration::from_secs(1)).unwrap();
}

fn get_keyboard() -> Option<Device<GlobalContext>> {
    rusb::devices()
        .unwrap()
        .iter()
        .find(|e|e.device_descriptor().unwrap().vendor_id() == VENDOR_ID )
}