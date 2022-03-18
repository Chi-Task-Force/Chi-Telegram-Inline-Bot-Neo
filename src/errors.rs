use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    #[error("mongodb error: {0}")]
    DB(#[from] mongodb::error::Error),
    #[error("telegram request error: {0}")]
    Telegram(#[from] teloxide::RequestError),
}
