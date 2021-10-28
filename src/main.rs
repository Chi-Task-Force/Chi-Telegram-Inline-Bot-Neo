#![allow(clippy::non_ascii_literal, clippy::cast_lossless)]

use std::env;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use itertools::Itertools;
use mongodb::Client;
use parking_lot::RwLock;
use teloxide::prelude::*;
use teloxide::types::{
    InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
};
use teloxide::utils::command::BotCommand;
use teloxide_listener::Listener;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::error;
use url::Url;

use errors::Result;

use crate::booking::Booking;
use crate::corpus::CorpusClient;
use crate::migrate::Migrator;
use crate::seller::Seller;
use crate::stats::MongoDBLogger;
use crate::utils::mask_user;

mod booking;
mod corpus;
mod errors;
mod migrate;
mod seller;
mod stats;
mod utils;

const BOT_NAME: &str = "realskyzh_bot";
const UPD_INTERVAL_SECS: u64 = 60 * 60;

#[derive(Debug, BotCommand)]
#[command(rename = "lowercase")]
enum Command {
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
    let dispatcher = Dispatcher::new(bot.clone())
        .inline_queries_handler({
            let logger = logger.clone();
            let seller = seller.clone();
            let booking = booking.clone();
            move |rx: DispatcherHandlerRx<AutoSend<Bot>, InlineQuery>| {
                UnboundedReceiverStream::new(rx).for_each_concurrent(None, move |query| {
                    let sell_count = logger
                        .stats()
                        .users
                        .get(&mask_user(query.update.from.id))
                        .copied()
                        .unwrap_or(0);
                    let sell_stat = format!(
                        "我已经卖了 {} 句菜{}",
                        sell_count,
                        if sell_count > 20 { "，我 zc" } else { "" }
                    );
                    let sell_stat_hash = format!("{:x}", md5::compute(&sell_stat));

                    // booking stat resp so we won't count it into user sell log
                    booking.write().book_stat(sell_stat_hash.clone());

                    let moan = seller.moan();
                    let moan_hash = format!("{:x}", md5::compute(&moan));

                    let answers = {
                        let mut booking = booking.write();
                        seller
                            .sell(query.update.query.as_str())
                            .into_iter()
                            .map(|s| {
                                let hash = format!("{:x}", md5::compute(&s));
                                booking.book_answer(hash.clone(), s.clone());
                                (hash, s)
                            })
                            .collect_vec()
                    };

                    let results = vec![InlineQueryResultArticle::new(
                        moan_hash,
                        "菜喘",
                        InputMessageContent::Text(InputMessageContentText::new(moan)),
                    )]
                        .into_iter()
                        .chain(answers.into_iter().map(|(hash, s)| {
                            InlineQueryResultArticle::new(
                                hash,
                                s.clone(),
                                InputMessageContent::Text(InputMessageContentText::new(s)),
                            )
                        }))
                        .chain(vec![InlineQueryResultArticle::new(
                            sell_stat_hash,
                            "卖菜统计",
                            InputMessageContent::Text(InputMessageContentText::new(sell_stat)),
                        )])
                        .map(InlineQueryResult::Article)
                        .collect_vec();

                    async move {
                        let resp = query
                            .requester
                            .answer_inline_query(&query.update.id, results)
                            .is_personal(true)
                            .cache_time(0)
                            .await;
                        if let Err(e) = resp {
                            error!("unable to send inline response: {:?}", e);
                        }
                    }
                })
            }
        })
        .chosen_inline_results_handler({
            let logger = logger.clone();
            let booking = booking;
            move |rx: DispatcherHandlerRx<AutoSend<Bot>, ChosenInlineResult>| {
                UnboundedReceiverStream::new(rx).for_each_concurrent(None, move |query| {
                    let logger = logger.clone();
                    let result_id = &query.update.result_id;

                    let stat_receipt = booking.write().check_stat(result_id);
                    let maybe_info = if stat_receipt {
                        None
                    } else {
                        let answer = booking
                            .read()
                            .get_answer(result_id.as_str())
                            .unwrap_or_else(|| String::from("-1"));
                        let user = mask_user(query.update.from.id);
                        Some((answer, user))
                    };

                    async move {
                        if let Some((answer, user)) = maybe_info {
                            let res = logger.log(answer, user).await;
                            if let Err(e) = res {
                                error!("unable to log chosen item: {:?}", e);
                            }
                        }
                    }
                })
            }
        })
        .messages_handler({
            let logger = logger;
            move |rx: DispatcherHandlerRx<AutoSend<Bot>, Message>| {
                UnboundedReceiverStream::new(rx).commands::<Command, _>(BOT_NAME).for_each_concurrent(None, move |(upd, command)| {
                    let answer = match command {
                        Command::Stat => {
                            let summary = logger.stats().summary();

                            let top_sentences_formatted = summary.top_sentences.into_iter().map(|(s, count)| format!("{}：{} 次", s, count)).join("\n");
                            format!("总共已经有 {} 名迟化人卖了 {} 句菜\n其中最迟的人卖了 {} 句\n\n被卖得最多次的句子：\n{}",
                                    summary.users,
                                    summary.total,
                                    summary.top_user_count,
                                    top_sentences_formatted
                            )
                        }
                    };

                    async move {
                        let resp = upd.answer(answer).await;
                        if let Err(e) = resp {
                            error!("unable to send stat answer: {:?}", e);
                        }
                    }
                })
            }
        });

    dispatcher
        .setup_ctrlc_handler()
        .dispatch_with_listener(
            Listener::from_env().build(bot).await,
            LoggingErrorHandler::with_custom_text("An error from the update listener"),
        )
        .await;

    Ok(())
}
