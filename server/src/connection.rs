use std::{io::{Read, Write}, net::TcpStream, os::unix::net::UnixStream, sync::{Arc, Mutex}};

use crate::AppResult;

pub struct Connection {
    pub reader: Arc<Mutex<ConnectionType<Reader>>>,
    writer: ConnectionType<Writer>,
}

// TODO REMOVE THESE MARKERS IF NOT NEEDED
pub struct Reader;
struct Writer;
pub struct ConnectionType<T> {
    inner: ConnectionVariant,
    _marker: std::marker::PhantomData<T>,
}

enum ConnectionVariant {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl Connection {
    /// Create a new connection from a TcpStream, will return an error if the stream fails to clone
    pub fn tcp(stream: TcpStream) -> AppResult<Self> {
        let reader = ConnectionType::new(ConnectionVariant::Tcp(stream.try_clone()?));
        let reader = Arc::new(Mutex::new(reader));

        Ok(Self {
            reader,
            writer: ConnectionType::new(ConnectionVariant::Tcp(stream)),
        })
    }

    /// Create a new connection from a UnixStream, will return an error if the stream fails to clone
    pub fn unix(stream: UnixStream) -> AppResult<Self> {
        let reader = ConnectionType::new(ConnectionVariant::Unix(stream.try_clone()?));
        let reader = Arc::new(Mutex::new(reader));

        Ok(Self {
            reader,
            writer: ConnectionType::new(ConnectionVariant::Unix(stream)),
        })
    }

    /// Returns the copy of the stream descriptor used for sending data
    pub fn writer(&mut self) -> &mut dyn Write {
        match &mut self.writer.inner {
            ConnectionVariant::Tcp(stream) => stream,
            ConnectionVariant::Unix(stream) => stream,
        }
    }

    /// Sets the non-blocking mode
    pub fn set_nonblocking(&self, v: bool) -> AppResult<()> {
        // The change will be applied to both the reader and writer
        match &self.writer.inner {
            ConnectionVariant::Tcp(stream) => stream.set_nonblocking(v)?,
            ConnectionVariant::Unix(stream) => stream.set_nonblocking(v)?,
        };

        Ok(())
    }
}

impl<T> ConnectionType<T> {
    fn new(variant: ConnectionVariant) -> Self {
        Self {
            inner: variant,
            _marker: std::marker::PhantomData,
        }
    }
}

impl ConnectionType<Reader> {
    /// Returns the copy of the stream descriptor used for receiving data
    pub fn reader(&mut self) -> &mut dyn Read {
        match &mut self.inner {
            ConnectionVariant::Tcp(stream) => stream,
            ConnectionVariant::Unix(stream) => stream,
        }
    }
}
