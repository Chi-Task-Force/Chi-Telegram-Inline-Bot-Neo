use parking_lot::{RwLock, RwLockReadGuard};
use reqwest::Client;
use url::Url;

use crate::errors::Result;

#[derive(Debug, Clone)]
pub struct Corpus {
    pub common: Vec<String>,
    pub refuse: Vec<String>,
    pub trigger: Vec<String>,
    pub phrase: Vec<Vec<String>>,
}

#[derive(Debug)]
pub struct CorpusClient {
    client: Client,
    base_url: Url,
    corpus: RwLock<Corpus>
}

async fn fetch(client: &Client, url: Url) -> Result<Vec<String>> {
    Ok(client
        .get(url.as_str())
        .send()
        .await?
        .text()
        .await?
        .trim()
        .split('\n')
        .map(ToString::to_string)
        .collect())
}

impl CorpusClient {
    pub async fn new_with_url(base_url: &Url) -> Result<Self> {
        let client = Client::new();

        let common = fetch(&client, base_url.join("common.txt").unwrap()).await?;
        let refuse = fetch(&client, base_url.join("refuse.txt").unwrap()).await?;
        let trigger = fetch(&client, base_url.join("trigger.txt").unwrap()).await?;
        let phrase = fetch(&client, base_url.join("phrase.txt").unwrap())
            .await?
            .into_iter()
            .map(|s| s.split(' ').map(ToString::to_string).collect())
            .collect();

        let corpus = Corpus {
            common,
            refuse,
            trigger,
            phrase
        };

        Ok(Self {
            client,
            base_url: base_url.clone(),
            corpus: RwLock::new(corpus)
        })
    }
    pub async fn update(&self) -> Result<()> {
        let base_url = &self.base_url;
        let client = &self.client;

        let common = fetch(client, base_url.join("common.txt").unwrap()).await?;
        let refuse = fetch(client, base_url.join("refuse.txt").unwrap()).await?;
        let trigger = fetch(client, base_url.join("trigger.txt").unwrap()).await?;
        let phrase = fetch(client, base_url.join("phrase.txt").unwrap())
            .await?
            .into_iter()
            .map(|s| s.split(' ').map(ToString::to_string).collect())
            .collect();

        let mut corpus = self.corpus.write();
        corpus.common = common;
        corpus.refuse = refuse;
        corpus.trigger = trigger;
        corpus.phrase = phrase;

        Ok(())
    }
    pub fn corpus(&self) -> RwLockReadGuard<Corpus> {
        self.corpus.read()
    }
}
