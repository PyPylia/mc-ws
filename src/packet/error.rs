use super::deserialize_packet;

#[derive(Debug, Default, Clone)]
pub struct ErrorPacket {
    pub status_message: String,
    pub status_code: i32,
}

deserialize_packet!(
    ErrorPacket; "error",
    body "statusMessage" => String: status_message,
    body "statusCode" => i32: status_code,
);
