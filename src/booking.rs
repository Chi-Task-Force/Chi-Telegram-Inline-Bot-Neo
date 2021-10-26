use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct Booking {
    stats: HashSet<String>,
    answers: HashMap<String, String>,
}

impl Booking {
    pub fn check_stat(&mut self, hash: &str) -> bool {
        self.stats.remove(hash)
    }
    pub fn get_answer(&self, hash: &str) -> Option<String> {
        self.answers.get(hash).cloned()
    }
    pub fn book_stat(&mut self, hash: String) {
        self.stats.insert(hash);
    }
    pub fn book_answer(&mut self, hash: String, answer: String) {
        self.answers.insert(hash, answer);
    }
}
