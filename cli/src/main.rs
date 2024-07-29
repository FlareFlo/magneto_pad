mod kb_handle;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use rusb::{Device, GlobalContext};
use shared::message::Message;
use shared::VENDOR_ID;

fn main() {
    let device = get_keyboard().unwrap();
    println!("Found keyboard! Bus {:03} Device {:03}", device.bus_number(), device.address());

    sudo::escalate_if_needed().unwrap();
    println!("Escalating to sudo to set device permissions");
    let f = fs::File::open(format!("/dev/bus/usb/{:03}/{:03}", device.bus_number(), device.address())).unwrap();
    let mut perms = f.metadata().unwrap().permissions();
    perms.set_mode(0o0666);
    f.set_permissions(perms).unwrap();

    let kb = device.open().unwrap();
    kb.claim_interface(2).unwrap();
    kb.write_bulk(2, Message::Ping.serialize().as_slice(), Duration::from_secs(1)).unwrap();
    let mut buf = [0; 128];
    let len = kb.read_bulk(130, &mut buf, Duration::from_secs(0)).unwrap();
    dbg!(Message::deserialize(&buf[..len]));
}

fn get_keyboard() -> Option<Device<GlobalContext>> {
    rusb::devices()
        .unwrap()
        .iter()
        .find(|e|e.device_descriptor().unwrap().vendor_id() == VENDOR_ID )
}