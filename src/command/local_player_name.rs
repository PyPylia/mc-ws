use super::Command;
use crate::{
    packet::{CommandRequestPacket, CommandResponsePacket},
    Error, Result,
};

pub struct LocalPlayerNameCommand;
pub struct LocalPlayerNameCommandResponse {
    pub name: String,
}

impl Command for LocalPlayerNameCommand {
    type Response = LocalPlayerNameCommandResponse;
}

impl From<LocalPlayerNameCommand> for CommandRequestPacket {
    fn from(_: LocalPlayerNameCommand) -> Self {
        Self::new("getlocalplayername")
    }
}

impl TryFrom<CommandResponsePacket> for LocalPlayerNameCommandResponse {
    type Error = Error;

    fn try_from(value: CommandResponsePacket) -> Result<Self> {
        Ok(Self {
            name: value
                .extra_data
                .get("localplayername")
                .ok_or(Error::MissingField("localplayername"))?
                .as_str()
                .ok_or(Error::InvalidType)?
                .to_string(),
        })
    }
}
