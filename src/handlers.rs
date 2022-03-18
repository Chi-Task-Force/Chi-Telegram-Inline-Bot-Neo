use std::sync::Arc;

use itertools::Itertools;
use parking_lot::RwLock;
use teloxide::adaptors::AutoSend;
use teloxide::payloads::AnswerInlineQuerySetters;
use teloxide::requests::Requester;
use teloxide::types::{
    ChosenInlineResult, InlineQuery, InlineQueryResult, InlineQueryResultArticle,
    InputMessageContent, InputMessageContentText, Message,
};
use teloxide::Bot;

use crate::errors::Error;
use crate::{mask_user, Booking, Command, MongoDBLogger, Seller};

pub async fn inline_query_handler(
    query: InlineQuery,
    bot: AutoSend<Bot>,
    logger: Arc<MongoDBLogger>,
    seller: Seller,
    booking: Arc<RwLock<Booking>>,
) -> Result<(), Error> {
    let sell_count = logger
        .stats()
        .users
        .get(&mask_user(query.from.id))
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
            .sell(query.query.as_str())
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

    bot.answer_inline_query(&query.id, results)
        .is_personal(true)
        .cache_time(0)
        .await?;
    Ok(())
}

pub async fn chosen_inline_handler(
    query: ChosenInlineResult,
    logger: Arc<MongoDBLogger>,
    booking: Arc<RwLock<Booking>>,
) -> Result<(), Error> {
    let logger = logger.clone();
    let result_id = &query.result_id;

    let stat_receipt = booking.write().check_stat(result_id);
    let maybe_info = if stat_receipt {
        None
    } else {
        let answer = booking
            .read()
            .get_answer(result_id.as_str())
            .unwrap_or_else(|| String::from("-1"));
        let user = mask_user(query.from.id);
        Some((answer, user))
    };

    if let Some((answer, user)) = maybe_info {
        logger.log(answer, user).await?;
    }
    Ok(())
}

pub async fn message_handler(
    command: Command,
    msg: Message,
    bot: AutoSend<Bot>,
    logger: Arc<MongoDBLogger>,
) -> Result<(), Error> {
    let answer = match command {
        Command::Stat => {
            let summary = logger.stats().summary();

            let top_sentences_formatted = summary
                .top_sentences
                .into_iter()
                .map(|(s, count)| format!("{}：{} 次", s, count))
                .join("\n");
            format!("总共已经有 {} 名迟化人卖了 {} 句菜\n其中最迟的人卖了 {} 句\n\n被卖得最多次的句子：\n{}",
                    summary.users,
                    summary.total,
                    summary.top_user_count,
                    top_sentences_formatted
            )
        }
    };

    bot.send_message(msg.chat.id, answer).await?;
    Ok(())
}
