//! 应用共享状态：数据库连接池 + 只读配置。

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<Config>,
}

impl AppState {
    pub fn new(pool: SqlitePool, cfg: Arc<Config>) -> Self {
        Self { pool, cfg }
    }
}