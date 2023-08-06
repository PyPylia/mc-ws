pub mod help;
mod local_player_name;
mod say;

pub use help::{HelpCommand, HelpCommandResponse};
pub use local_player_name::*;
pub use say::*;

use crate::{
    packet::{CommandRequestPacket, CommandResponsePacket},
    Error,
};

pub trait Command: Into<CommandRequestPacket>
where
    Self::Response: TryFrom<CommandResponsePacket, Error = Error>,
{
    type Response;
}
