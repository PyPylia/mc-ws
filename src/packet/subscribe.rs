use super::serialize_packet;
use crate::event::EventType;

#[derive(Debug, Default, Clone, Copy)]
pub struct SubscribePacket {
    pub event_name: EventType,
}

serialize_packet!(
    SubscribePacket; "subscribe",
    body "eventName" => EventType: event_name,
    header "messageType" => String: "commandRequest",
);
