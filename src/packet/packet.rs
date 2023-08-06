use std::{collections::HashMap, fmt};

use super::{
    CommandRequestPacket, CommandResponsePacket, ErrorPacket, EventPacket, SubscribePacket,
    UnsubscribePacket,
};
use crate::packet::{get_unexpected, DeserializablePacket};
use serde::{
    de::{MapAccess, Unexpected, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Packet {
    Error(ErrorPacket),
    Subscribe(SubscribePacket),
    Unsubscribe(UnsubscribePacket),
    Event(EventPacket),
    CommandRequest(CommandRequestPacket),
    CommandResponse(CommandResponsePacket),
}

impl Serialize for Packet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        match self {
            Packet::Subscribe(value) => value.serialize(serializer),
            Packet::Unsubscribe(value) => value.serialize(serializer),
            Packet::CommandRequest(value) => value.serialize(serializer),
            _ => Err(Error::custom("unserializable packet")),
        }
    }
}

struct PacketVisitor;
impl<'de> Visitor<'de> for PacketVisitor {
    type Value = Packet;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("packet")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        use serde::de::Error;
        let mut packet: HashMap<String, Value> = HashMap::new();

        while let Some((key, value)) = map.next_entry()? {
            packet.insert(key, value);
        }

        let purpose = match packet.get("header").ok_or(Error::missing_field("header"))? {
            Value::Object(header) => {
                match header.get("messagePurpose").ok_or(Error::missing_field(
                    "header.messagePurpose",
                ))? {
                    Value::String(value) => value,
                    other => {
                        return Err(Error::invalid_value(
                            get_unexpected(other),
                            &"string",
                        ))
                    }
                }
            }
            other => {
                return Err(Error::invalid_value(
                    get_unexpected(other),
                    &"json object",
                ))
            }
        };

        Ok(match purpose.as_str() {
            "commandResponse" => Packet::CommandResponse(
                <CommandResponsePacket as DeserializablePacket<A>>::deserialize_map(packet)?,
            ),
            "error" => Packet::Error(<ErrorPacket as DeserializablePacket<
                A,
            >>::deserialize_map(packet)?),
            "event" => Packet::Event(<EventPacket as DeserializablePacket<
                A,
            >>::deserialize_map(packet)?),
            other => {
                return Err(Error::invalid_value(
                    Unexpected::Str(other),
                    &"packet type",
                ))
            }
        })
    }
}

impl<'de> Deserialize<'de> for Packet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PacketVisitor)
    }
}
