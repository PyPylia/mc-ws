use super::serialize_packet;
use crate::event::EventType;

#[derive(Debug, Default, Clone, Copy)]
pub struct UnsubscribePacket {
    pub event_name: EventType,
}

serialize_packet!(
    UnsubscribePacket; "unsubscribe",
    body "eventName" => EventType: event_name,
    header "messageType" => String: "commandRequest",
);
