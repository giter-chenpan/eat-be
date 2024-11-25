use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Dishes::Table)
                .if_not_exists()
                .col(ColumnDef::new(Dishes::Id).integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Dishes::Name).string().not_null())
                .col(ColumnDef::new(Dishes::Desc).string().not_null())
                .col(ColumnDef::new(Dishes::ViewUrl).string().not_null())
                .col(ColumnDef::new(Dishes::CategoryId).integer().not_null())
                .col(ColumnDef::new(Dishes::CreateTime).timestamp().not_null())
                .col(ColumnDef::new(Dishes::UpdateTime).timestamp().not_null())
                .col(ColumnDef::new(Dishes::Status).string().not_null())
                .col(ColumnDef::new(Dishes::CreateUser).string().not_null())
                .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Dishes::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Dishes {
    Table,
    Id,
    Name,
    Desc,
    ViewUrl,
    CategoryId,
    CreateTime,
    UpdateTime,
    Status,
    CreateUser,
}
