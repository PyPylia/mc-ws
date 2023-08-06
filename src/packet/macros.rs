use serde::de::{MapAccess, Unexpected};
use serde_json::{Number, Value};
use std::{any::type_name, collections::HashMap};

pub(super) trait DeserializablePacket<'de, A: MapAccess<'de>>
where
    Self: Sized,
{
    fn deserialize_map(map: HashMap<String, Value>) -> Result<Self, A::Error>;
}

pub(super) fn get_name<T>() -> String {
    let mut name = String::new();

    for c in type_name::<T>().chars() {
        if c.is_uppercase() {
            name.extend(c.to_lowercase());
            name.push(' ')
        } else {
            name.push(c)
        }
    }

    name
}

pub(super) fn get_unexpected_number<'a>(number: &Number) -> Unexpected<'a> {
    number.as_u64().map_or(
        number.as_i64().map_or(
            number.as_f64().map_or(
                Unexpected::Other("unknown number"),
                |value| Unexpected::Float(value),
            ),
            |value| Unexpected::Signed(value),
        ),
        |value| Unexpected::Unsigned(value),
    )
}

pub(super) fn get_unexpected<'a>(value: &'a Value) -> Unexpected<'a> {
    match value {
        Value::Null => Unexpected::Option,
        Value::Array(_) => Unexpected::Seq,
        Value::Bool(value) => Unexpected::Bool(*value),
        Value::Number(value) => get_unexpected_number(value),
        Value::String(value) => Unexpected::Str(value.as_str()),
        Value::Object(_) => Unexpected::Map,
    }
}

macro_rules! select_map {
    ($header:expr, $body:expr, body) => {
        $body
    };
    ($header:expr, $body:expr, header) => {
        $header
    };
}

macro_rules! serialize_packet {
    (@serialize $self:ident, $header:expr, $body:expr,) => {};
    (@extract_value $self:ident $field:ident) => {
        $self.$field.clone()
    };
    (@extract_value $self:ident $const:literal) => {
        $const
    };
    (@serialize $self:ident, $header:expr, $body:expr, other_body => $field:ident,) => {
        $body.append(&mut $self.clone().$field);
    };
    (@serialize $self:ident, $header:expr, $body:expr, $map_id:ident $key:literal => Option<$type:ty>: $field:ident, $($tail:tt)*) => {
        match $self.$field.clone() {
            Some(value) => {
                $crate::packet::select_map!($header, $body, $map_id).insert(
                    $key.to_string(),
                    ::serde_json::to_value(value)
                        .ok()
                        .ok_or(::serde::ser::Error::custom("failed to serialize field"))?
                );
            }
            None => {}
        };
        serialize_packet!(@serialize $self, $header, $body, $($tail)*);
    };
    (@serialize $self:ident, $header:expr, $body:expr, $map_id:ident $key:literal => $type:ty: $value:tt, $($tail:tt)*) => {
        $crate::packet::select_map!($header, $body, $map_id).insert(
            $key.to_string(),
            ::serde_json::to_value(
                serialize_packet!(@extract_value $self $value)
            ).ok().ok_or(::serde::ser::Error::custom("failed to serialize field"))?
        );
        serialize_packet!(@serialize $self, $header, $body, $($tail)*);
    };
    ($type:ident; $purpose:expr, $($tail:tt)*) => {
        impl ::serde::ser::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
                where
                    S: ::serde::ser::Serializer,
                {
                use ::serde::ser::SerializeMap;

                let mut map = serializer.serialize_map(None)?;
                let mut header = ::serde_json::Map::new();
                let mut body = ::serde_json::Map::new();

                serialize_packet!(@serialize self, header, body, $($tail)*);

                header.insert("messagePurpose".to_string(), ::serde_json::Value::String($purpose.to_string()));
                header.insert("version".to_string(), ::serde_json::Value::Number(1.into()));
                if !header.contains_key("requestId") {
                    header.insert("requestId".to_string(), ::serde_json::Value::String(::uuid::Uuid::new_v4().to_string()));
                }

                map.serialize_entry("header", &::serde_json::Value::Object(header))?;
                map.serialize_entry("body", &::serde_json::Value::Object(body))?;

                map.end()
            }
        }
    };
}

macro_rules! deserialize_packet {
    (@deserialize $self:ident, $header:expr, $body:expr,) => {};
    (@extract_map $header:expr, $body:expr, body) => {
        $body
    };
    (@extract_map $header:expr, $body:expr, header) => {
        $header
    };
    (@use_value $self:ident, $field:ident, $value:expr) => {
        $self.$field = $value;
    };
    (@use_value $self:ident, $const:literal, $value:expr) => {
        ($value == $const)
            .then_some(())
            .ok_or(::serde::de::Error::invalid_value(
                ::serde::de::Unexpected::Other($value.to_string().as_str()),
                &$const.to_string().as_str(),
            ))?;
    };
    (@deserialize $self:ident, $header:expr, $body:expr, other_body => $field:ident,) => {
        $self.$field = $body;
    };
    (@deserialize $self:ident, $header:expr, $body:expr, $map_id:ident $key:literal => Option<$type:ty>: $field:ident, $($tail:tt)*) => {
        $self.$field = match $crate::packet::select_map!($header, $body, $map_id).remove($key) {
            Some(value) => Some(
                ::serde_json::from_value::<$type>(value)
                    .ok()
                    .ok_or(::serde::de::Error::custom(
                        format!("failed to deserialize field: {} (Option<{}>)", stringify!($key), stringify!($type)).as_str()
                    ))?
            ),
            None => None,
        };
        deserialize_packet!(@deserialize $self, $header, $body, $($tail)*);
    };
    (@deserialize $self:ident, $header:expr, $body:expr, $map_id:ident $key:literal => $type:ty: $value:tt, $($tail:tt)*) => {
        deserialize_packet!(@use_value $self, $value,
            ::serde_json::from_value::<$type>(
                $crate::packet::select_map!($header, $body, $map_id)
                    .remove($key)
                    .ok_or(::serde::de::Error::missing_field(
                        concat!(stringify!($map_id), ".", $key)
                    ))?
            ).ok().ok_or(
                ::serde::de::Error::custom(
                    format!("failed to deserialize field: {} ({})", stringify!($key), stringify!($type)).as_str()
                )
            )?
        );
        deserialize_packet!(@deserialize $self, $header, $body, $($tail)*);
    };
    ($type:ident; $purpose:expr, $($tail:tt)*) => {
        impl<'de, A: ::serde::de::MapAccess<'de>> $crate::packet::DeserializablePacket<'de, A> for $type where
            Self: ::std::marker::Sized,
        {
            #[allow(unused_mut)]
            fn deserialize_map(
                mut packet: ::std::collections::HashMap<::std::string::String, ::serde_json::Value>,
            ) -> ::std::result::Result<Self, A::Error> {
            let mut header =
                match packet
                    .remove("header")
                    .ok_or(::serde::de::Error::missing_field(
                        "header",
                    ))? {
                    ::serde_json::Value::Object(value) => value,
                    value => {
                        return ::std::result::Result::Err(::serde::de::Error::invalid_type(
                            $crate::packet::get_unexpected(&value),
                            &"json object",
                        ))
                    }
                };

            let mut body = match packet
                .remove("body")
                .ok_or(::serde::de::Error::missing_field(
                    "body",
                ))? {
                ::serde_json::Value::Object(value) => value,
                value => {
                    return ::std::result::Result::Err(::serde::de::Error::invalid_type(
                        $crate::packet::get_unexpected(&value),
                        &"json object",
                    ))
                }
            };

            match header
                .get("version")
                .ok_or(::serde::de::Error::missing_field(
                    "header.version",
                ))? {
                ::serde_json::Value::Number(_) => (),
                value => {
                    return ::std::result::Result::Err(::serde::de::Error::invalid_type(
                        $crate::packet::get_unexpected(value),
                        &"i64",
                    ))
                }
            };

            match header
                .get("messagePurpose")
                .ok_or(::serde::de::Error::missing_field(
                    "header.messagePurpose",
                ))? {
                ::serde_json::Value::String(value) => {
                    (value == $purpose)
                        .then_some(())
                        .ok_or(::serde::de::Error::invalid_value(
                            ::serde::de::Unexpected::Str(value),
                            &$purpose,
                        ))?
                }
                value => {
                    return ::std::result::Result::Err(::serde::de::Error::invalid_type(
                        $crate::packet::get_unexpected(value),
                        &"string",
                    ))
                }
            };

            let mut value = Self::default();
            deserialize_packet!(@deserialize value, header, body, $($tail)*);

            ::std::result::Result::Ok(value)
            }
        }

        impl<'de> ::serde::de::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct Visitor;
                impl<'de> ::serde::de::Visitor<'de> for Visitor {
                    type Value = $type;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str(&$crate::packet::get_name::<$type>())
                    }

                    fn visit_map<A>(self, mut map: A) -> ::std::result::Result<Self::Value, A::Error>
                    where
                        A: ::serde::de::MapAccess<'de>,
                    {
                        let mut packet: ::std::collections::HashMap<
                            ::std::string::String,
                            ::serde_json::Value,
                        > = ::std::collections::HashMap::new();

                        while let ::std::option::Option::Some((key, value)) = map.next_entry()? {
                            packet.insert(key, value);
                        }

                        <$type as $crate::packet::DeserializablePacket<A>>::deserialize_map(packet)
                    }
                }

                deserializer.deserialize_map(Visitor)
            }
        }
    }
}

pub(super) use deserialize_packet;
pub(super) use select_map;
pub(super) use serialize_packet;
