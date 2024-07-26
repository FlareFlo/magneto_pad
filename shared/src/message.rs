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
#[non_exhaustive]
pub enum Message {
	Command {
		cmd: Command,
		id: u32,
	},
	Response {
		res: Response,
		to: u32,
	}
}

#[derive(Debug, Encode, Decode, PartialEq)]
pub enum Command {
	Heartbeat(u32),
	ToggleLed,
}

#[derive(Debug, Encode, Decode, PartialEq)]
pub enum Response {
	Heartbeat(u32),
}

impl Message {
	pub fn request_heartbeat(h: u32) -> Self {
		Self::make_command(Command::Heartbeat(h))
	}

	pub fn toggle_led() -> Self {
		Self::make_command(Command::ToggleLed)
	}

	pub fn command(self) -> Option<Command> {
		match self {
			Message::Command { cmd, .. } => {Some(cmd)}
			Message::Response { .. } => {None}
		}
	}

	fn make_command(cmd: Command) -> Self {
		Self::Command {
			cmd,
			id: 0,
		}
	}

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