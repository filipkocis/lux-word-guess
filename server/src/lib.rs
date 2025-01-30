mod protocol;
mod connection;
mod server;

pub use protocol::*;
pub use connection::*;
pub use server::*;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Serde(serde_json::Error),
    InvalidCommand,
    InvalidAuth,
    TooLarge,
    InvalidConnection,
    Unauthorized,
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Io(value)
    }
}
