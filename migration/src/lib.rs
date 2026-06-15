pub use sea_orm_migration::prelude::*;

// Legacy MySQL-only incremental migrations (modify_column) — kept for reference.
#[allow(dead_code)]
mod m20240415_080239_create_table;
#[allow(dead_code)]
mod m20240821_020525_create_table;
#[allow(dead_code)]
mod m20241122_025249_create_table;
#[allow(dead_code)]
mod m20241125_024818_create_userselect_table;
#[allow(dead_code)]
mod m20241125_071756_update_dishes_table;
#[allow(dead_code)]
mod m20241125_075345_create_image_table;
#[allow(dead_code)]
mod m20241125_080427_update_dishes_table_view_url;
#[allow(dead_code)]
mod m20250416_064621_create_table;
#[allow(dead_code)]
mod m20250417_065651_create_words_table;
#[allow(dead_code)]
mod m20250417_072954_update_words_table;
#[allow(dead_code)]
mod m20250417_073805_update_words_table;
#[allow(dead_code)]
mod m20250418_090001_update_words_table_id;
#[allow(dead_code)]
mod m20250513_065155_update_words_table;
#[allow(dead_code)]
mod m20250513_082952_create_table;
#[allow(dead_code)]
mod m20250523_031934_update_table_words_id;
#[allow(dead_code)]
mod m20250523_032419_update_table_words_id;
#[allow(dead_code)]
mod m20250523_032800_update_table_words_transtion;
#[allow(dead_code)]
mod m20250718_073733_update_user_table;
#[allow(dead_code)]
mod m20250723_105707_update_user_table_salt;
#[allow(dead_code)]
mod m20250723_110308_update_user_table_salt;
#[allow(dead_code)]
mod m20250723_110634_delete_user_table_salt;
#[allow(dead_code)]
mod m20251226_071535_create_table;

mod m20260611_000000_sqlite_baseline;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260611_000000_sqlite_baseline::Migration)]
    }
}
