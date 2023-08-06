use std::collections::HashMap;

use super::Command;
use crate::{
    packet::{CommandRequestPacket, CommandResponsePacket},
    Error, Result,
};

#[derive(Debug)]
pub enum BedrockCommandArgument {
    Literal,
    Optional(String),
    Required(String),
}

#[derive(Debug)]
pub struct BedrockCommandSchema {
    pub name: String,
    pub arguments: HashMap<String, BedrockCommandArgument>,
}

fn process_typed_arg<I: Iterator<Item = char>>(
    char_iter: &mut I,
    end_char: char,
    key: &mut String,
) -> Option<String> {
    let arg: String = char_iter.take_while(|c| *c != end_char).collect();
    let (key_str, arg_type) = arg.split_once(": ")?;
    char_iter.next();
    key.push_str(key_str);
    Some(arg_type.to_string())
}

impl BedrockCommandSchema {
    fn from_str(value: &str) -> Option<Self> {
        let mut arguments = HashMap::new();
        let (name, args) = value.get(1..)?.split_once(' ')?;
        let mut arg_chars = args.chars();

        while let Some(first_char) = arg_chars.next() {
            let mut key = String::new();

            let value = match first_char {
                '<' => BedrockCommandArgument::Required(process_typed_arg(
                    &mut arg_chars,
                    '>',
                    &mut key,
                )?),
                '[' => BedrockCommandArgument::Optional(process_typed_arg(
                    &mut arg_chars,
                    ']',
                    &mut key,
                )?),
                first_char => {
                    key.push(first_char);
                    key.push_str(
                        &(&mut arg_chars)
                            .take_while(|c| *c != ' ')
                            .collect::<String>(),
                    );
                    BedrockCommandArgument::Literal
                }
            };

            arguments.insert(key.to_string(), value);
        }

        Some(Self {
            name: name.to_string(),
            arguments,
        })
    }
}

pub struct HelpCommand {
    pub page: u32,
}

pub struct HelpCommandResponse {
    pub body: String,
    pub page: u32,
    pub page_count: u32,
}

impl HelpCommandResponse {
    pub fn get_commands(&self) -> Vec<BedrockCommandSchema> {
        let mut commands = vec![];

        for line in self.body.split("\n") {
            match BedrockCommandSchema::from_str(line) {
                Some(value) => commands.push(value),
                None => {}
            }
        }

        commands
    }
}

impl Command for HelpCommand {
    type Response = HelpCommandResponse;
}

impl Default for HelpCommand {
    fn default() -> Self {
        Self { page: 1 }
    }
}

impl From<HelpCommand> for CommandRequestPacket {
    fn from(value: HelpCommand) -> Self {
        Self::new(format!("help {}", value.page).as_str())
    }
}

impl TryFrom<CommandResponsePacket> for HelpCommandResponse {
    type Error = Error;

    fn try_from(value: CommandResponsePacket) -> Result<Self> {
        Ok(Self {
            body: value
                .extra_data
                .get("body")
                .ok_or(Error::MissingField("body"))?
                .as_str()
                .ok_or(Error::InvalidType)?
                .to_string(),
            page: value
                .extra_data
                .get("page")
                .ok_or(Error::MissingField("page"))?
                .as_u64()
                .ok_or(Error::InvalidType)? as u32,
            page_count: value
                .extra_data
                .get("pageCount")
                .ok_or(Error::MissingField("pageCount"))?
                .as_u64()
                .ok_or(Error::InvalidType)? as u32,
        })
    }
}
