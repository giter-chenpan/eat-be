pub use sea_orm_migration::prelude::*;

mod m20240415_080239_create_table;
mod m20240821_020525_create_table;
mod m20241122_025249_create_table;
mod m20241125_024818_create_userselect_table;
mod m20241125_071756_update_dishes_table;
mod m20241125_075345_create_image_table;
mod m20241125_080427_update_dishes_table_view_url;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240415_080239_create_table::Migration),
            Box::new(m20240821_020525_create_table::Migration),
            Box::new(m20241122_025249_create_table::Migration),
            Box::new(m20241125_024818_create_userselect_table::Migration),
            Box::new(m20241125_071756_update_dishes_table::Migration),
            Box::new(m20241125_075345_create_image_table::Migration),
            Box::new(m20241125_080427_update_dishes_table_view_url::Migration),
        ]
    }
}
