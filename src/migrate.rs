use std::collections::HashMap;
use std::io::Read;

use mongodb::Database;
use serde::Deserialize;

use crate::errors::Result;
use crate::stats::{Sentence, Total, User};

#[derive(Debug, Clone, Deserialize)]
pub struct Migrator {
    total: u64,
    per_sentence: HashMap<String, u64>,
    per_user: HashMap<String, u64>,
}

impl Migrator {
    pub fn from_reader(f: impl Read) -> Result<Self> {
        serde_json::from_reader(f).map_err(std::convert::Into::into)
    }
    pub async fn migrate(self, db: Database) -> Result<()> {
        let coll_total = db.collection("stats");
        let coll_sentences = db.collection("sentences");
        let coll_users = db.collection("users");

        coll_total
            .insert_one(Total { total: self.total }, None)
            .await?;
        coll_sentences
            .insert_many(
                self.per_sentence.into_iter().map(|(k, v)| Sentence {
                    sentence: k,
                    count: v,
                }),
                None,
            )
            .await?;
        coll_users
            .insert_many(
                self.per_user
                    .into_iter()
                    .map(|(k, v)| User { user: k, count: v }),
                None,
            )
            .await?;

        Ok(())
    }
}
