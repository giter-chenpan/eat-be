use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(UserSelect::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(UserSelect::Id)
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key()
                )
                .col(ColumnDef::new(UserSelect::UserId).string().not_null())
                .col(ColumnDef::new(UserSelect::DishId).string().not_null())
                .col(ColumnDef::new(UserSelect::CreateTime).timestamp().not_null())
                .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(UserSelect::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum UserSelect {
    Table,
    Id,
    UserId,
    DishId,
    CreateTime,
}
