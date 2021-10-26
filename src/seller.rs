use std::sync::Arc;

use either::Either;
use rand::{random, thread_rng};
use rand::seq::IteratorRandom;
use rand::seq::SliceRandom;

use crate::corpus::CorpusClient;

#[rustfmt::skip]
const SEP: [&str; 16] = [
    "…", "…", "…", "…", "…", "…",
    "……", "……", "……", "……",
    "………", "………",
    "！", "！！",
    "、、", "、、、",
];

#[rustfmt::skip]
const MOAN: [&str; 35] = [
    "啊", "啊", "啊", "啊", "啊",
    "啊啊", "啊啊", "啊啊", "啊啊",
    "啊啊啊", "啊啊啊", "啊啊啊",
    "嗯", "嗯", "嗯", "嗯",
    "嗯嗯", "嗯嗯",
    "唔", "唔",
    "唔嗯", "唔嗯",
    "唔哇", "唔哇",
    "哇啊", "哇啊啊",
    "好舒服", "好棒", "继续", "用力", "不要停",
    "不要", "那里不可以", "好变态", "要坏掉啦",
];

fn random_sep() -> &'static str {
    let mut rng = thread_rng();
    SEP.choose(&mut rng).unwrap()
}

fn random_moan() -> &'static str {
    let mut rng = thread_rng();
    MOAN.choose(&mut rng).unwrap()
}

fn random_text() -> String {
    let mut text = random_moan().to_string();
    while text.chars().count() < 20 && random::<f32>() < 0.25 {
        text.push_str(random_sep());
        text.push_str(random_moan());
    }
    text
}

#[derive(Debug, Clone)]
pub struct Seller {
    client: Arc<CorpusClient>,
}

impl Seller {
    pub fn new(client: Arc<CorpusClient>) -> Self {
        Seller { client }
    }
}

impl Seller {
    pub fn sell(&self, keyword: &str) -> Vec<String> {
        let corpus = self.client.corpus();
        let mut rng = thread_rng();
        if keyword.is_empty() {
            Either::Left(corpus.common.iter())
        } else {
            Either::Right(
                corpus
                    .common
                    .iter()
                    .filter(|s| s.contains(keyword)),
            )
        }
            .cloned()
            .choose_multiple(&mut rng, 5)
    }
    pub fn moan(&self) -> String {
        let corpus = self.client.corpus();
        let mut rng = thread_rng();
        let count = (1..=3).choose(&mut rng).unwrap();
        let phrase_set = corpus.phrase.iter().choose(&mut rng).unwrap();
        let vegetables = phrase_set.choose_multiple(&mut rng, count);

        vegetables.into_iter().fold(String::new(), |mut x, acc| {
            x.push_str(acc);
            x.push_str(random_sep());
            x.push_str(random_text().as_str());
            x.push_str(random_sep());
            x
        })
    }
}
