use super::{Event, EventType};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum MessageType {
    Chat,
    Say,
    Tell,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayerMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub sender: String,
    pub receiver: String,
}

impl Event for PlayerMessage {
    fn get_type() -> EventType {
        EventType::PlayerMessage
    }
}
