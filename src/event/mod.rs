mod player_message;
mod types;

pub use player_message::PlayerMessage;
pub use types::EventType;

use crate::{
    packet::{EventPacket, Packet, UnsubscribePacket},
    Error, Result,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    marker::PhantomData,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::WatchStream, StreamExt};

pub trait Event: DeserializeOwned {
    fn get_type() -> EventType;
}

pub struct EventListener<T: Event> {
    ref_count: Arc<AtomicU32>,
    packet_sender: mpsc::Sender<Packet>,
    event_receiver: WatchStream<EventPacket>,
    _phantom: PhantomData<T>,
}

impl<T: Event> EventListener<T> {
    pub fn get_type(&self) -> EventType {
        T::get_type()
    }

    pub(crate) fn new_unchecked(
        ref_count: Arc<AtomicU32>,
        packet_sender: mpsc::Sender<Packet>,
        event_receiver: WatchStream<EventPacket>,
    ) -> EventListener<T> {
        EventListener {
            ref_count,
            packet_sender,
            event_receiver,
            _phantom: PhantomData,
        }
    }

    pub async fn recv(&mut self) -> Result<T> {
        while let Some(event) = self.event_receiver.next().await {
            if event.event_name == T::get_type() {
                return Ok(serde_json::from_value(Value::Object(
                    event.properties,
                ))?);
            }
        }

        Err(Error::StreamExhausted("event"))
    }
}

impl<T: Event> Drop for EventListener<T> {
    fn drop(&mut self) {
        let val = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        if val == 1 {
            self.packet_sender
                .try_send(Packet::Unsubscribe(UnsubscribePacket {
                    event_name: T::get_type(),
                }))
                .ok();
        }
    }
}
