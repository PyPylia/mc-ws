use super::{deserialize_packet, JsonObject};
use crate::event::EventType;

#[derive(Debug, Default, Clone)]
pub struct EventPacket {
    pub event_name: EventType,
    pub(crate) properties: JsonObject,
}

deserialize_packet!(
    EventPacket; "event",
    header "eventName" => EventType: event_name,
    other_body => properties,
);
