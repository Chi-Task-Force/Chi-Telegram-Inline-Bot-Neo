use std::collections::HashMap;

use futures_util::{StreamExt, TryStreamExt};
use itertools::Itertools;
use mongodb::{Client, Collection};
use mongodb::bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use parking_lot::{RwLock, RwLockReadGuard};
use serde::Deserialize;

use crate::errors::Result;

#[derive(Debug, Deserialize)]
struct Total {
    total: u64,
}

#[derive(Debug, Deserialize)]
struct User {
    user: String,
    count: u64,
}

#[derive(Debug, Deserialize)]
struct Sentence {
    sentence: String,
    count: u64,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub total: u64,
    pub users: u64,
    pub top_sentences: HashMap<String, u64>,
    pub top_user_count: u64,
}

#[derive(Debug, Clone)]
pub struct Stat {
    pub total: u64,
    pub sentences: HashMap<String, u64>,
    pub users: HashMap<String, u64>,
}

impl Stat {
    pub fn summary(&self) -> Summary {
        let top_sentences = self
            .sentences
            .iter()
            .sorted_by_key(|item| -(*item.1 as i128))
            .take(5)
            .map(|item| {
                if item.0 == "-1" {
                    (String::from("菜喘"), *item.1)
                } else {
                    (item.0.clone(), *item.1)
                }
            })
            .collect();
        let top_user_count = self
            .users
            .iter()
            .sorted_by_key(|item| -(*item.1 as i128))
            .take(1)
            .map(|item| *item.1)
            .find_or_first(|_| true)
            .unwrap_or(0);
        Summary {
            total: self.total,
            users: self.users.len() as u64,
            top_sentences,
            top_user_count,
        }
    }
}

#[derive(Debug)]
pub struct MongoDBLogger {
    coll_total: Collection<Total>,
    coll_sentences: Collection<Sentence>,
    coll_users: Collection<User>,
    stats: RwLock<Stat>,
}

async fn fetch_stats(
    total: &Collection<Total>,
    sentences: &Collection<Sentence>,
    users: &Collection<User>,
) -> Result<Stat> {
    let total = total
        .find_one_and_update(
            doc! {"total": {"$exists": true}},
            doc! {"$setOnInsert": {"total": 0}},
            FindOneAndUpdateOptions::builder()
                .upsert(true)
                .return_document(ReturnDocument::After)
                .build(),
        )
        .await?
        .unwrap();
    let sentences: HashMap<_, _> = sentences
        .find(doc! {"sentences": {"$exists": true}}, None)
        .await?
        .map(|item| item.map(|sentence| (sentence.sentence, sentence.count)))
        .try_collect()
        .await?;
    let users: HashMap<_, _> = users
        .find(doc! {"users": {"$exists": true}}, None)
        .await?
        .map(|item| item.map(|user| (user.user, user.count)))
        .try_collect()
        .await?;
    Ok(Stat {
        total: total.total,
        sentences,
        users,
    })
}

impl MongoDBLogger {
    pub async fn new(uri: &str, db_name: &str) -> Result<Self> {
        let client = Client::with_uri_str(uri).await?;
        let db = client.database(db_name);
        let coll_total = db.collection("stats");
        let coll_sentences = db.collection("sentences");
        let coll_users = db.collection("users");
        let stats = fetch_stats(&coll_total, &coll_sentences, &coll_users).await?;
        Ok(Self {
            coll_total,
            coll_sentences,
            coll_users,
            stats: RwLock::new(stats),
        })
    }
    pub async fn sync(&self) -> Result<()> {
        let new_stats =
            fetch_stats(&self.coll_total, &self.coll_sentences, &self.coll_users).await?;

        let mut stats = self.stats.write();
        *stats = new_stats;

        Ok(())
    }
    pub fn stats(&self) -> RwLockReadGuard<Stat> {
        self.stats.read()
    }
    pub async fn log(&self, sentence: String, user: String) -> Result<()> {
        let config = FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(ReturnDocument::After)
            .build();
        let total = self
            .coll_total
            .find_one_and_update(
                doc! {
                    "total": {"$exists": true}
                },
                doc! {
                    "$inc": {"total": 1}
                },
                config.clone(),
            )
            .await?
            .unwrap();
        let sentence = self
            .coll_sentences
            .find_one_and_update(
                doc! {
                    "sentence": sentence
                },
                doc! {
                    "$inc": {"count": 1}
                },
                config.clone(),
            )
            .await?
            .unwrap();
        let user = self
            .coll_users
            .find_one_and_update(
                doc! {
                    "user": user
                },
                doc! {
                    "$inc": {"count": 1}
                },
                config,
            )
            .await?
            .unwrap();

        let mut stats = self.stats.write();
        stats.total = total.total;
        stats.sentences.insert(sentence.sentence, sentence.count);
        stats.users.insert(user.user, user.count);

        Ok(())
    }
}
