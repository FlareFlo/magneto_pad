use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::{Publisher, PubSubChannel, Subscriber};
use shared::message::Message;

pub type UsbChannel = PubSubChannel<NoopRawMutex, (bool, Message), 10, 2, 2>;

pub type UsbSubscriber<'a> = Subscriber<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;
pub type UsbPublisher<'a> = Publisher<'a, NoopRawMutex, (bool, Message), 10, 2, 2>;