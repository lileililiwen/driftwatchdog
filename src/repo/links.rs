//! `manual_links` repository placeholder. Populated by the
//! `correlation-and-ai-context` change.

use crate::repo::Db;

pub struct Links<'a> {
    #[allow(dead_code)]
    db: &'a Db,
}

impl<'a> Links<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }
}
