use std::str::Utf8Error;

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("reqwest error: {0}")]
    Payload(#[from] reqwest::Error),
    #[error("encoding error: {0}")]
    Encoding(#[from] Utf8Error),
    #[error("mongodb error: {0}")]
    DB(#[from] mongodb::error::Error),
}
