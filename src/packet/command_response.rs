use super::{deserialize_packet, JsonObject};
use uuid::Uuid;

#[derive(Debug, Default, Clone)]
pub struct CommandResponsePacket {
    pub status_code: i32,
    pub status_message: Option<String>,
    pub request_id: Uuid,
    pub extra_data: JsonObject,
}

deserialize_packet!(
    CommandResponsePacket; "commandResponse",
    body "statusCode" => i32: status_code,
    body "statusMessage" => Option<String>: status_message,
    header "requestId" => Uuid: request_id,
    other_body => extra_data,
);
