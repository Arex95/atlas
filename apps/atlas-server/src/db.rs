use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info};

pub async fn init_db(database_url: &str) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let path_str = database_url
        .trim_start_matches("sqlite:///")
        .trim_start_matches("sqlite://");

    let db_path = Path::new(path_str);
    if let Some(parent) = db_path.parent()
        && !parent.exists()
    {
        info!("[DB] Creating database directory: {:?}", parent);
        std::fs::create_dir_all(parent)?;
    }

    info!("[DB] Connecting to database: {}", database_url);

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect_with(options)
        .await?;

    info!("[DB] Running migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| {
            error!("[DB] Migration error: {:?}", e);
            e
        })?;

    info!("[DB] Initialization complete ✓");
    Ok(pool)
}
