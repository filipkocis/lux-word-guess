use std::io::Write;

use crate::AppResult;

use super::{Command, WriteBytes};

#[derive(Debug)]
pub struct Packet {
    pub payload: Command,
}

impl Packet {
    pub fn new(command: Command) -> Self {
        Self {
            payload: command,
        }
    }
}

impl WriteBytes for Packet {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        let mut temp = Vec::new();
        let np = self.payload.write(&mut temp)?;

        let nh = u16::write(&(temp.len() as u16), buffer)?;
        buffer.write_all(&temp)?;

        Ok(nh + np)
    }
}
