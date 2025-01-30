pub mod command;
pub mod packet;

use std::io::Write;

pub use command::*;
pub use packet::*;

use crate::AppResult;

pub trait WriteBytes {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize>;
}

pub trait ReadBytes {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized;
}

impl WriteBytes for &str {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        let len = self.len();
        let nl = (len as u16)
            .write(buffer)?;

        buffer.write_all(self.as_bytes())?;

        Ok(nl + len)
    }
}

impl WriteBytes for String {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        self.as_str().write(buffer) 
    }
}

impl ReadBytes for String {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized {
        let length = u16::read(buffer)?;
        let bytes = buffer.take(length as usize)
            .map(|b| *b).collect::<Vec<_>>();

        let string = String::from_utf8_lossy(&bytes);
        Some(string.to_string())
    }
}

impl<T: WriteBytes> WriteBytes for [T] {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        let nl = (self.len() as u16)
            .write(buffer)?;

        let n: usize = self.iter()
            .map(|t| t.write(buffer))
            .collect::<Result<Vec<_>, _>>()?
            .iter().sum();

        Ok(nl + n)
    }
}

impl<T: ReadBytes> ReadBytes for Vec<T> {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized {
        let array_length = u16::read(buffer)?; 

        (0..array_length)
            .map(|_| T::read(buffer))
            .collect()
    }
}

impl ReadBytes for u8 {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized {
        let n = *buffer.next()?;
        Some(n)
    }
}

impl WriteBytes for u16 {
    fn write(&self, buffer: &mut dyn Write) -> AppResult<usize> {
        buffer.write(&self.to_be_bytes())?;
        Ok(2)
    }
}

impl ReadBytes for u16 {
    fn read(buffer: &mut std::slice::Iter<u8>) -> Option<Self> where Self: Sized {
        let b1 = *buffer.next()?;
        let b2 = *buffer.next()?;

        let n = u16::from_be_bytes([b1, b2]);
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nums() {
        let mut buffer = Vec::new();

        let n = 42u16;
        n.write(&mut buffer).unwrap();

        let parsed = u16::read(&mut buffer.iter()).unwrap();

        assert_eq!(n, parsed);
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_string() {
        let mut buffer = Vec::new();

        let text = "hello";
        text.write(&mut buffer).unwrap();

        let parsed = String::read(&mut buffer.iter()).unwrap();

        assert_eq!(text, parsed);
        assert_eq!(buffer.len(), text.len() + 2);
    }

    #[test]
    fn test_array() {
        let mut buffer = Vec::new();

        let array = ["foo", "bar", "baz"];
        array.write(&mut buffer).unwrap();

        let parsed = Vec::<String>::read(&mut buffer.iter()).unwrap();

        let calc_len = array.iter().map(|s| s.len() + 2).sum::<usize>() + 2;

        assert_eq!(array.to_vec(), parsed);
        assert_eq!(buffer.len(), calc_len);
    }
}
