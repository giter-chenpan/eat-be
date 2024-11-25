use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(DishesImages::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(DishesImages::Id)
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key()
                )
                .col(ColumnDef::new(DishesImages::Name).string().not_null())
                .col(ColumnDef::new(DishesImages::CreateTime).timestamp().not_null())
                .col(ColumnDef::new(DishesImages::UpdateTime).timestamp().not_null())
                .col(ColumnDef::new(DishesImages::ImageData).blob(BlobSize::Long))
                .to_owned()
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(DishesImages::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum DishesImages {
    Table,
    Id,
    Name,
    CreateTime,
    UpdateTime,
    ImageData,
}
