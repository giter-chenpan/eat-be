use sea_orm::ConnectOptions;
use sea_orm_rocket::{ rocket::figment::Figment, Config, Database };
use std::time::Duration;
use rocket_db_pools::{ Database as RedisDatabase, deadpool_redis };

#[derive(Database, Debug)]
#[database("sea_orm")]
pub struct Db(SeaOrmPool);

#[derive(Debug, Clone)]
pub struct SeaOrmPool {
    pub conn: sea_orm::DatabaseConnection,
}

#[derive(RedisDatabase)]
#[database("redis")]
pub struct RedisPool(deadpool_redis::Pool);

#[async_trait]
impl sea_orm_rocket::Pool for SeaOrmPool {
    type Error = sea_orm::DbErr;

    type Connection = sea_orm::DatabaseConnection;

    async fn init(figment: &Figment) -> Result<Self, Self::Error> {
        let config = figment.extract::<Config>().unwrap();
        let mut options: ConnectOptions = config.url.into();
        // Cap pool size: SQLite is file-based and serializes writes, so a large
        // pool mainly causes `SQLITE_BUSY`. Defaults: min 2 warm, max 10, idle
        // 10 min so stale conns recycle.
        let max_connections = config.max_connections.min(10) as u32;
        let min_connections = config.min_connections.unwrap_or(2);
        let idle_timeout = config.idle_timeout.unwrap_or(600);
        options
            .max_connections(max_connections)
            .min_connections(min_connections)
            .connect_timeout(Duration::from_secs(config.connect_timeout))
            .sqlx_logging(config.sqlx_logging)
            .idle_timeout(Duration::from_secs(idle_timeout));
        let conn = sea_orm::Database::connect(options).await?;

        Ok(SeaOrmPool { conn })
    }

    fn borrow(&self) -> &Self::Connection {
        &self.conn
    }
}
