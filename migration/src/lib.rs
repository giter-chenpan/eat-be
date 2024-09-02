pub use sea_orm_migration::prelude::*;

mod m20240415_080239_create_table;
mod m20240821_020525_create_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240415_080239_create_table::Migration),
            Box::new(m20240821_020525_create_table::Migration),
        ]
    }
}
