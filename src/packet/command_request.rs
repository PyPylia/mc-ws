use super::serialize_packet;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum OriginType {
    Player,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct Origin {
    #[serde(rename = "origin")]
    pub origin_type: OriginType,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            origin_type: OriginType::Player,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CommandRequestPacket {
    pub origin: Origin,
    pub command_line: String,
    pub request_id: Uuid,
}

impl CommandRequestPacket {
    pub fn new(command_line: &str) -> Self {
        Self {
            origin: Origin::default(),
            command_line: command_line.to_string(),
            request_id: Uuid::new_v4(),
        }
    }
}

serialize_packet!(
    CommandRequestPacket; "commandRequest",
    body "origin" => Origin: origin,
    body "commandLine" => String: command_line,
    header "messageType" => String: "commandRequest",
    header "requestId" => Uuid: request_id,
    header "version" => i32: 1,
);
