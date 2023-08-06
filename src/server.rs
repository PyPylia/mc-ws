use self::event_loop::{EventLoop, EventLoopChannels};
use crate::{
    command::Command,
    event::{Event, EventListener, EventType},
    packet::{CommandRequestPacket, CommandResponsePacket, EventPacket, Packet, SubscribePacket},
    Error, MultiError, MultiResult, Result,
};
use futures::{future::BoxFuture, task::noop_waker_ref, FutureExt};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, watch, Semaphore},
    task::JoinHandle,
};
use tokio_stream::wrappers::WatchStream;
use tokio_tungstenite::WebSocketStream;
use uuid::Uuid;

type SentCommand = (
    Uuid,
    oneshot::Sender<CommandResponsePacket>,
);

pub struct Server {
    loop_handle: JoinHandle<Result<()>>,
    event_receiver: watch::Receiver<EventPacket>,
    command_sender: mpsc::Sender<SentCommand>,
    packet_sender: mpsc::Sender<Packet>,
    subscribed_events: BTreeMap<EventType, Arc<AtomicU32>>,
    command_semaphore: Arc<Semaphore>,
}

impl Server {
    pub fn spawn<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        websocket: WebSocketStream<S>,
    ) -> Self {
        let EventLoopChannels {
            event_loop,
            event_receiver,
            command_sender,
            packet_sender,
        } = EventLoop::new(websocket);

        Self {
            loop_handle: event_loop.spawn(),
            event_receiver,
            command_sender,
            packet_sender,
            subscribed_events: BTreeMap::new(),
            command_semaphore: Arc::new(Semaphore::new(100)),
        }
    }

    pub async fn run<
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        H: for<'a> FnOnce(&'a mut Server) -> BoxFuture<'a, Result<()>>,
    >(
        websocket: WebSocketStream<S>,
        handler: H,
    ) -> MultiResult<()> {
        let mut server = Server::spawn(websocket);
        let handler_result = handler(&mut server).await;
        let loop_result = server.get_loop_result();
        drop(server);

        match (handler_result, loop_result) {
            (Ok(()), None) => Ok(()),
            (Ok(()), Some(loop_error)) => Err(MultiError::LoopErrored(loop_error)),
            (Err(handler_error), None) => Err(MultiError::HandlerErrored(
                handler_error,
            )),
            (Err(handler_error), Some(loop_error)) => Err(MultiError::BothErrored {
                loop_error,
                handler_error,
            }),
        }
    }

    pub fn is_running(&self) -> bool {
        !self.loop_handle.is_finished()
    }

    fn assert_running(&self) -> Result<()> {
        if self.loop_handle.is_finished() {
            Err(Error::LoopNotRunning)
        } else {
            Ok(())
        }
    }

    pub async fn recv_raw_event(&mut self) -> Result<EventPacket> {
        self.assert_running()?;
        self.event_receiver.borrow_and_update();
        self.event_receiver.changed().await?;
        Ok(self.event_receiver.borrow_and_update().clone())
    }

    pub async fn send_raw_command(
        &mut self,
        command: CommandRequestPacket,
    ) -> Result<CommandResponsePacket> {
        self.assert_running()?;

        let semaphore = self.command_semaphore.clone();
        let permit = semaphore.acquire().await;
        let uuid = command.request_id;

        self.packet_sender
            .send(Packet::CommandRequest(command))
            .await?;

        let (tx, rx) = oneshot::channel();
        self.command_sender.send((uuid, tx)).await?;

        let result = rx.await;
        drop(permit);
        Ok(result?)
    }

    pub async fn send_command<T: Command>(&mut self, request: T) -> Result<T::Response>
    where
        T::Response: TryFrom<CommandResponsePacket, Error = Error>,
    {
        let response = self.send_raw_command(request.into()).await?;
        if response.status_code == 0 {
            response.try_into()
        } else {
            Err(Error::MinecraftError {
                status_message: response.status_message,
                status_code: response.status_code,
            })
        }
    }

    pub async fn subscribe<T: Event>(&mut self) -> Result<EventListener<T>> {
        self.assert_running()?;

        let event_name = T::get_type();
        let ref_count = self
            .subscribed_events
            .entry(event_name)
            .or_insert_with(|| Arc::new(AtomicU32::new(0)))
            .clone();

        if ref_count.fetch_add(1, Ordering::SeqCst) == 0 {
            self.packet_sender
                .send(Packet::Subscribe(SubscribePacket {
                    event_name,
                }))
                .await?;
        }

        Ok(EventListener::new_unchecked(
            ref_count,
            self.packet_sender.clone(),
            WatchStream::from_changes(self.event_receiver.clone()),
        ))
    }

    pub fn get_loop_result(&mut self) -> Option<Error> {
        if let Poll::Ready(result) = self.loop_handle.poll_unpin(&mut Context::from_waker(
            noop_waker_ref(),
        )) {
            if let Ok(result) = result {
                return result.err();
            }
        }

        None
    }

    pub fn close(mut self) {
        self.loop_handle.abort();
        self.command_semaphore.close();
        self.subscribed_events.clear();
    }
}

mod event_loop {
    use super::SentCommand;
    use crate::{
        packet::{EventPacket, Packet},
        Error, Result,
    };
    use futures::{executor::block_on, SinkExt};
    use std::{borrow::Cow, time::Duration};
    use tokio::{
        io::{AsyncRead, AsyncWrite},
        sync::{mpsc, watch},
        task::JoinHandle,
        time::timeout,
    };
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use tokio_tungstenite::{
        tungstenite::{
            protocol::{frame::coding::CloseCode, CloseFrame},
            Message,
        },
        WebSocketStream,
    };

    const CHANNEL_SIZE: usize = u16::MAX as usize;

    pub struct EventLoopChannels<S: AsyncRead + AsyncWrite + Unpin> {
        pub event_loop: EventLoop<S>,
        pub event_receiver: watch::Receiver<EventPacket>,
        pub packet_sender: mpsc::Sender<Packet>,
        pub command_sender: mpsc::Sender<SentCommand>,
    }

    pub struct EventLoop<S: AsyncRead + AsyncWrite + Unpin> {
        sent_commands: Vec<SentCommand>,
        stream: WebSocketStream<S>,
        event_sender: watch::Sender<EventPacket>,
        packet_receiver: ReceiverStream<Packet>,
        command_receiver: ReceiverStream<SentCommand>,
    }

    impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> EventLoop<S> {
        fn process_message(message: Message) -> Result<Option<Packet>> {
            match message {
                Message::Text(text) => {
                    let packet = serde_json::from_str::<Packet>(text.as_str())?;
                    Ok(Some(packet))
                }
                _ => Ok(None),
            }
        }

        async fn handle_packet(&mut self, packet: Packet) -> Result<()> {
            match packet.clone() {
                Packet::Event(event) => self.event_sender.send(event).map_err(|err| err.into()),

                Packet::Error(error) => Err(Error::MinecraftError {
                    status_message: Some(error.status_message),
                    status_code: error.status_code,
                }),

                Packet::CommandResponse(response) => self
                    .sent_commands
                    .swap_remove(
                        self.sent_commands
                            .iter()
                            .position(|(id, _)| id == &response.request_id)
                            .ok_or(Error::UnexpectedPacket(packet))?,
                    )
                    .1
                    .send(response)
                    .ok()
                    .ok_or(Error::CommandHandlingError),

                _ => Err(Error::UnexpectedPacket(packet)),
            }
        }

        async fn event_loop(mut self) -> Result<()> {
            loop {
                tokio::select! {
                    biased;

                    command_future = self.command_receiver.next() => self.sent_commands.push(
                        command_future.ok_or(Error::StreamExhausted("command"))?
                    ),

                    message = self.stream.try_next() => {
                        if let Some(packet) = Self::process_message(
                            message?.ok_or(Error::StreamExhausted("websocket"))?
                        )? {
                            self.handle_packet(packet).await?;
                        }
                    },

                    packet = self.packet_receiver.next() =>
                        self.stream.send(Message::Text(serde_json::to_string(
                            &packet.ok_or(Error::StreamExhausted("packet"))?
                        )?)).await?,
                }
            }
        }

        pub fn new_from_raw(
            stream: WebSocketStream<S>,
            event_sender: watch::Sender<EventPacket>,
            packet_receiver: ReceiverStream<Packet>,
            command_receiver: ReceiverStream<SentCommand>,
        ) -> Self {
            Self {
                sent_commands: vec![],
                stream,
                event_sender,
                packet_receiver,
                command_receiver,
            }
        }

        pub fn new(stream: WebSocketStream<S>) -> EventLoopChannels<S> {
            let (event_tx, mut event_rx) = watch::channel(EventPacket::default());
            let (command_tx, command_rx) = mpsc::channel(CHANNEL_SIZE);
            let (packet_tx, packet_rx) = mpsc::channel(CHANNEL_SIZE);

            event_rx.borrow_and_update();

            EventLoopChannels {
                event_loop: Self::new_from_raw(
                    stream,
                    event_tx,
                    packet_rx.into(),
                    command_rx.into(),
                ),
                event_receiver: event_rx.into(),
                packet_sender: packet_tx,
                command_sender: command_tx,
            }
        }

        pub fn spawn(self) -> JoinHandle<Result<()>> {
            tokio::spawn(self.event_loop())
        }
    }

    impl<S: AsyncRead + AsyncWrite + Unpin> Drop for EventLoop<S> {
        fn drop(&mut self) {
            self.command_receiver.close();
            self.packet_receiver.close();

            let result = block_on(timeout(
                Duration::from_millis(10),
                self.stream.close(Some(CloseFrame {
                    code: CloseCode::Away,
                    reason: Cow::Borrowed("Connection closing"),
                })),
            ));

            if let Err(_) = result {
                eprintln!("websocket could not gracefully close in time.");
            }
        }
    }
}
