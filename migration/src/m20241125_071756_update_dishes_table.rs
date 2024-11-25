use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_table(Dishes::Table.to_string()).await.unwrap() {
            manager.alter_table(
                Table::alter()
                    .table(Dishes::Table)
                    .add_column(ColumnDef::new(Dishes::ViewId).string().not_null())
                    .to_owned()
            ).await
        } else {
            Ok(())
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Dishes::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Dishes {
    Table,
    ViewId,
}
