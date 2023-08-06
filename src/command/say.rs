use super::Command;
use crate::{
    packet::{CommandRequestPacket, CommandResponsePacket},
    Error, Result,
};

pub struct SayCommand {
    pub message: String,
}

impl SayCommand {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

pub struct SayCommandResponse {
    pub message: String,
}

impl Command for SayCommand {
    type Response = SayCommandResponse;
}

impl From<SayCommand> for CommandRequestPacket {
    fn from(value: SayCommand) -> Self {
        Self::new(format!("say {}", value.message).as_str())
    }
}

impl TryFrom<CommandResponsePacket> for SayCommandResponse {
    type Error = Error;

    fn try_from(value: CommandResponsePacket) -> Result<Self> {
        Ok(Self {
            message: value
                .extra_data
                .get("message")
                .ok_or(Error::MissingField("message"))?
                .as_str()
                .ok_or(Error::InvalidType)?
                .to_string(),
        })
    }
}
