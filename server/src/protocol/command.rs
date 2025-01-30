use std::io::Write;

use crate::AppResult;

use super::{ReadBytes, WriteBytes};

#[derive(Clone, Debug)]
pub enum Command {
    Ok,
    Error(String),
    PasswordRequest,
    Password(String),
    SetId(String),
    OpponentsRequest,
    Opponents(Vec<String>),
    RequestMatch(String),
    PlayerJoined(String),
    Guess(String),
    Hint(String),
    Surrender,
    Win,
    RequestMatchGuess,
    SetGuess(String),
    PlayerLeft,

    SubscribeToGames(String),
    Unknown(String),
}

impl WriteBytes for Command {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        match self {
            Command::Ok => Self::write_byte(0, buffer),
            Command::Error(err) => Self::write_string_with_id(1, err, buffer),
            Command::PasswordRequest => Self::write_byte(2, buffer),
            Command::Password(password) => Self::write_string_with_id(3, password, buffer),
            Command::SetId(id) => Self::write_string_with_id(4, id, buffer),
            Command::OpponentsRequest => Self::write_byte(5, buffer),
            Command::Opponents(opponents) => {
                let b = Self::write_byte(6, buffer)?;
                let n = opponents.as_slice().write(buffer)?;
                Ok(b + n)
            },
            Command::RequestMatch(id) => Self::write_string_with_id(7, id, buffer),
            Command::PlayerJoined(id) => Self::write_string_with_id(8, id, buffer),
            Command::Guess(guess) => Self::write_string_with_id(9, guess, buffer),
            Command::Hint(hint) => Self::write_string_with_id(10, hint, buffer),
            Command::Surrender => Self::write_byte(11, buffer),
            Command::Win => Self::write_byte(12, buffer),
            Command::RequestMatchGuess => Self::write_byte(13, buffer),
            Command::SetGuess(guess) => Self::write_string_with_id(14, guess, buffer),
            Command::PlayerLeft => Self::write_byte(15, buffer),

            Command::SubscribeToGames(password) => Self::write_string_with_id(254, password, buffer),
            Command::Unknown(message) => Self::write_string_with_id(255, message, buffer),
        }
    }
}

impl Command {
    /// Helper function to write a single byte.
    fn write_byte(byte: u8, buffer: &mut dyn Write) -> AppResult<usize> {
        buffer.write_all(&[byte])?;
        Ok(1)
    }

    /// Helper function to write a string with the given command ID preceeding it.
    fn write_string_with_id(command_id: u8, string: &str, buffer: &mut dyn Write) -> AppResult<usize> {
        let b = Self::write_byte(command_id, buffer)?;
        let n = string.write(buffer)?;

        Ok(b + n)
    }
}

impl ReadBytes for Command {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized {
        let type_byte = u8::read(buffer)?;

        let command = match type_byte {
            0 => Command::Ok,
            1 => {
                let message = String::read(buffer)?;
                Command::Error(message)
            },
            2 => Command::PasswordRequest,
            3 => {
                let password = String::read(buffer)?;
                Command::Password(password)
            },
            4 => {
                let id = String::read(buffer)?;
                Command::SetId(id)
            },
            5 => Command::OpponentsRequest,
            6 => {
                let lobbies = <Vec<String>>::read(buffer)?;
                Command::Opponents(lobbies)
            },
            7 => {
                let id = String::read(buffer)?;
                Command::RequestMatch(id)
            },
            8 => {
                let id = String::read(buffer)?;
                Command::PlayerJoined(id)
            },
            9 => {
                let guess = String::read(buffer)?;
                Command::Guess(guess)
            },
            10 => {
                let hint = String::read(buffer)?;
                Command::Hint(hint)
            },
            11 => Command::Surrender,
            12 => Command::Win,
            13 => Command::RequestMatchGuess,
            14 => {
                let guess = String::read(buffer)?;
                Command::SetGuess(guess)
            },
            15 => Command::PlayerLeft,

            254 => {
                let password = String::read(buffer)?;
                Command::SubscribeToGames(password)
            },
            255 => {
                let error = String::read(buffer)?;
                Command::Unknown(error)
            }
            _ => return None
        };

        Some(command)
    }
}
