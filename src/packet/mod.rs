mod command_request;
mod command_response;
mod error;
mod event;
mod macros;
mod packet;
mod subscribe;
mod unsubscribe;

use serde_json::{Map, Value};

pub use command_request::{CommandRequestPacket, Origin, OriginType};
pub use command_response::CommandResponsePacket;
pub use error::ErrorPacket;
pub use event::EventPacket;
pub(self) use macros::*;
pub use packet::Packet;
pub use subscribe::SubscribePacket;
pub use unsubscribe::UnsubscribePacket;

pub(self) type JsonObject = Map<String, Value>;
