#![allow(
    clippy::non_ascii_literal,
    clippy::cast_lossless,
    clippy::module_name_repetitions
)]

use std::env;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use mongodb::Client;
use parking_lot::RwLock;
use teloxide::dispatching2::{Dispatcher, HandlerExt, UpdateFilterExt};
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::requests::RequesterExt;
use teloxide::types::Update;
use teloxide::utils::command::BotCommand;
use teloxide::{dptree, Bot};
use teloxide_listener::Listener;
use tracing::error;
use url::Url;

use errors::Result;

use crate::booking::Booking;
use crate::corpus::CorpusClient;
use crate::handlers::{chosen_inline_handler, inline_query_handler, message_handler};
use crate::migrate::Migrator;
use crate::seller::Seller;
use crate::stats::MongoDBLogger;
use crate::utils::mask_user;

mod booking;
mod corpus;
mod errors;
mod handlers;
mod migrate;
mod seller;
mod stats;
mod utils;

const UPD_INTERVAL_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, BotCommand)]
#[command(rename = "lowercase")]
pub enum Command {
    Stat,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let base_url = Url::from_str(
        env::var("APP_CORPUS_URL")
            .expect("missing corpus url")
            .as_str(),
    )
    .expect("malformed url");
    let mongodb_uri = env::var("APP_MONGODB_URI").expect("missing mongodb url");
    let mongodb_db_name = env::var("APP_MONGODB_DBNAME").expect("missing mongodb dbname");
    let client = Client::with_uri_str(mongodb_uri).await?;
    let db = client.database(mongodb_db_name.as_str());

    let migrate_log = env::var("APP_MIGRATE_LOG").ok();
    if let Some(migrate_log) = migrate_log {
        let f = File::open(migrate_log)?;
        let migrator = Migrator::from_reader(f)?;
        migrator.migrate(db).await?;
        return Ok(());
    }

    let corpus = Arc::new(CorpusClient::new_with_url(&base_url).await?);
    let seller = Arc::new(Seller::new(corpus.clone()));

    let logger = Arc::new(MongoDBLogger::new(db).await?);

    let booking = Arc::new(RwLock::new(Booking::default()));

    {
        let corpus = corpus.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(UPD_INTERVAL_SECS)).await;
                if let Err(e) = corpus.update().await {
                    error!("unable to update corpus: {:?}", e);
                }
            }
        });
    }

    {
        let logger = logger.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(UPD_INTERVAL_SECS)).await;
                if let Err(e) = logger.sync().await {
                    error!("unable to sync logger: {:?}", e);
                }
            }
        });
    }

    let bot = Bot::from_env().auto_send();
    let listener = Listener::from_env_with_prefix("APP_")
        .build(bot.clone())
        .await;
    Dispatcher::builder(
        bot,
        dptree::entry()
            .branch(Update::filter_inline_query().endpoint(inline_query_handler))
            .branch(Update::filter_chosen_inline_result().endpoint(chosen_inline_handler))
            .branch(
                Update::filter_message()
                    .filter_command::<Command>()
                    .branch(dptree::endpoint(message_handler)),
            ),
    )
    .dependencies(dptree::deps![seller, logger, booking])
    .build()
    .setup_ctrlc_handler()
    .dispatch_with_listener(listener, LoggingErrorHandler::new())
    .await;

    Ok(())
}
