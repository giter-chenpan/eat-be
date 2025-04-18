use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Words::Table)
                .if_not_exists()
                .col(ColumnDef::new(Words::Id).integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Words::Word).string().not_null())
                .col(ColumnDef::new(Words::Translation).string().not_null())
                .col(ColumnDef::new(Words::CreateUser).string().not_null())
                .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Words::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Words {
    Table,
    Id,
    Word,
    Translation,
    CreateUser,
}
