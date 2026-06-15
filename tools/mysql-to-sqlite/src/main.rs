//! One-time tool: copy all data from MySQL to a fresh SQLite database.
//!
//! Usage:
//!   MYSQL_URL='mysql://user:pass@host:3306/eatbe' \
//!   SQLITE_URL='sqlite://data/eatbe.db?mode=rwc' \
//!   cargo run -p mysql-to-sqlite
//!
//! Set FORCE=1 to overwrite an existing SQLite file.

use entity::{
    category, dishes, dishes_images, times, user, user_select, words,
};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Set,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mysql_url = env::var("MYSQL_URL").expect("MYSQL_URL must be set (source MySQL database)");
    let sqlite_url = env::var("SQLITE_URL").unwrap_or_else(|_| "sqlite://data/eatbe.db?mode=rwc".into());
    let force = env::var("FORCE").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    if let Some(path) = sqlite_file_path(&sqlite_url) {
        if path.exists() {
            if force {
                std::fs::remove_file(&path)?;
                println!("Removed existing SQLite file: {}", path.display());
            } else {
                eprintln!(
                    "SQLite file already exists: {}. Set FORCE=1 to overwrite.",
                    path.display()
                );
                std::process::exit(1);
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    println!("Creating SQLite schema...");
    run_sqlite_migrations(&sqlite_url)?;

    println!("Connecting to MySQL...");
    let mysql = Database::connect(&mysql_url).await?;
    println!("Connecting to SQLite...");
    let sqlite = Database::connect(&sqlite_url).await?;

    let user_count = copy_users(&mysql, &sqlite).await?;
    let category_count = copy_categories(&mysql, &sqlite).await?;
    let dishes_count = copy_dishes(&mysql, &sqlite).await?;
    let images_count = copy_dishes_images(&mysql, &sqlite).await?;
    let user_select_count = copy_user_select(&mysql, &sqlite).await?;
    let words_count = copy_words(&mysql, &sqlite).await?;
    let times_count = copy_times(&mysql, &sqlite).await?;

    reset_autoincrement(&sqlite, "user").await?;
    reset_autoincrement(&sqlite, "category").await?;
    reset_autoincrement(&sqlite, "dishes").await?;
    reset_autoincrement(&sqlite, "dishes_images").await?;
    reset_autoincrement(&sqlite, "user_select").await?;
    reset_autoincrement(&sqlite, "times").await?;

    println!("\nMigration complete:");
    println!("  user:          {user_count} rows");
    println!("  category:      {category_count} rows");
    println!("  dishes:        {dishes_count} rows");
    println!("  dishes_images: {images_count} rows");
    println!("  user_select:   {user_select_count} rows");
    println!("  words:         {words_count} rows");
    println!("  times:         {times_count} rows");
    println!("\nUpdate Rocket.toml databases.sea_orm.url to:\n  {sqlite_url}");

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_sqlite_migrations(sqlite_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("cargo")
        .args(["run", "-p", "migration", "--quiet"])
        .current_dir(project_root())
        .env("DATABASE_URL", sqlite_url)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err("SQLite schema migration failed".into())
    }
}

fn sqlite_file_path(url: &str) -> Option<std::path::PathBuf> {
    let path = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))?;
    let path = path.split('?').next()?.trim_start_matches('/');
    if path.is_empty() {
        return None;
    }
    Some(Path::new(path).to_path_buf())
}

async fn copy_users(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = user::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        user::ActiveModel {
            id: Set(row.id),
            name: Set(row.name),
            password: Set(row.password),
            create_time: Set(row.create_time),
            salt: Set(row.salt),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_categories(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = category::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        category::ActiveModel {
            id: Set(row.id),
            name: Set(row.name),
            desc: Set(row.desc),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_dishes(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = dishes::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        dishes::ActiveModel {
            id: Set(row.id),
            name: Set(row.name),
            desc: Set(row.desc),
            view_url: Set(row.view_url),
            category_id: Set(row.category_id),
            create_time: Set(row.create_time),
            update_time: Set(row.update_time),
            status: Set(row.status),
            create_user: Set(row.create_user),
            view_id: Set(row.view_id),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_dishes_images(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = dishes_images::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        dishes_images::ActiveModel {
            id: Set(row.id),
            name: Set(row.name),
            create_time: Set(row.create_time),
            update_time: Set(row.update_time),
            image_data: Set(row.image_data),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_user_select(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = user_select::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        user_select::ActiveModel {
            id: Set(row.id),
            user_id: Set(row.user_id),
            dish_id: Set(row.dish_id),
            create_time: Set(row.create_time),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_words(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = words::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        words::ActiveModel {
            id: Set(row.id),
            word: Set(row.word),
            translation: Set(row.translation),
            create_user: Set(row.create_user),
            r#type: Set(row.r#type),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn copy_times(mysql: &DatabaseConnection, sqlite: &DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    let rows = times::Entity::find().all(mysql).await?;
    let count = rows.len();
    for row in rows {
        times::ActiveModel {
            id: Set(row.id),
            time: Set(row.time),
            all_time: Set(row.all_time),
            created_user: Set(row.created_user),
            created_time: Set(row.created_time),
        }
        .insert(sqlite)
        .await?;
    }
    Ok(count)
}

async fn reset_autoincrement(db: &DatabaseConnection, table: &str) -> Result<(), sea_orm::DbErr> {
    let sql = format!(
        "INSERT OR REPLACE INTO sqlite_sequence (name, seq) \
         SELECT '{table}', IFNULL(MAX(id), 0) FROM \"{table}\""
    );
    db.execute_unprepared(&sql).await?;
    Ok(())
}
