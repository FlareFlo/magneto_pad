use core::sync::atomic::{AtomicU32, Ordering};
use musli::{Decode, Encode, FixedBytes, Options, options};
use musli::options::{ByteOrder, Integer};
use musli::wire::Encoding;

const OPTIONS: Options = options::new()
	.with_integer(Integer::Fixed)
	.with_byte_order(ByteOrder::NETWORK)
	.build();

const ENCODING: Encoding<OPTIONS> = Encoding::new().with_options();

pub const MESSAGE_BUF_SIZE: usize = 64;

static NEXT_MSG_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, PartialEq, Encode, Decode)]
pub enum Message {
	Heartbeat,
	ToggleDebugLed,
}

impl Message {
	pub fn serialize(&self) -> FixedBytes<MESSAGE_BUF_SIZE> {
		let mut buf = FixedBytes::new();
		ENCODING.encode(&mut buf, self).unwrap();
		buf
	}

	pub fn deserialize(buf: &[u8]) -> Self {
		ENCODING.decode(buf).unwrap()
	}
}

#[cfg(test)]
mod test {
	use crate::message::Message;

	#[test]
	fn test_simple() {
		let msg = Message::request_heartbeat(42);
		let ser = msg.serialize();

		let dec = Message::deserialize(ser.as_slice());
		assert_eq!(dec, msg)
	}
}