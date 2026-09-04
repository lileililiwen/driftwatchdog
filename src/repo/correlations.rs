//! `correlations` repository placeholder. Populated by the
//! `correlation-and-ai-context` change.

use crate::repo::Db;

pub struct Correlations<'a> {
    #[allow(dead_code)]
    db: &'a Db,
}

impl<'a> Correlations<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }
}
