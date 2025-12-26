use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Times::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Times::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Times::Time).string().not_null())
                    .col(ColumnDef::new(Times::AllTime).string().not_null())
                    .col(ColumnDef::new(Times::CreatedUser).string().not_null())
                    .col(ColumnDef::new(Times::CreatedTime).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Times::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Times {
    Table,
    Id,
    Time,
    AllTime,
    CreatedUser,
    CreatedTime,
}
