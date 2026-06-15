use sea_orm_migration::prelude::*;

/// Consolidated schema for SQLite. Replaces the incremental MySQL migrations
/// which use unsupported `modify_column` operations on SQLite.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(User::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(User::Name).string().not_null())
                    .col(ColumnDef::new(User::Password).string().not_null())
                    .col(ColumnDef::new(User::CreateTime).date_time().not_null())
                    .col(ColumnDef::new(User::Salt).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Category::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Category::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Category::Name).string().not_null())
                    .col(ColumnDef::new(Category::Desc).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Dishes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Dishes::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Dishes::Name).string().not_null())
                    .col(ColumnDef::new(Dishes::Desc).string().not_null())
                    .col(ColumnDef::new(Dishes::ViewUrl).string().null())
                    .col(ColumnDef::new(Dishes::CategoryId).integer().not_null())
                    .col(ColumnDef::new(Dishes::CreateTime).timestamp().not_null())
                    .col(ColumnDef::new(Dishes::UpdateTime).timestamp().not_null())
                    .col(ColumnDef::new(Dishes::Status).string().not_null())
                    .col(ColumnDef::new(Dishes::CreateUser).string().not_null())
                    .col(ColumnDef::new(Dishes::ViewId).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(DishesImages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DishesImages::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DishesImages::Name).string().not_null())
                    .col(
                        ColumnDef::new(DishesImages::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DishesImages::UpdateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DishesImages::ImageData).blob(BlobSize::Blob(None)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserSelect::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserSelect::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserSelect::UserId).string().not_null())
                    .col(ColumnDef::new(UserSelect::DishId).string().not_null())
                    .col(
                        ColumnDef::new(UserSelect::CreateTime)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Words::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Words::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Words::Word).string().not_null())
                    .col(ColumnDef::new(Words::Translation).text().not_null())
                    .col(ColumnDef::new(Words::CreateUser).string().not_null())
                    .col(
                        ColumnDef::new(Words::Type)
                            .string()
                            .not_null()
                            .default("zh"),
                    )
                    .to_owned(),
            )
            .await?;

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
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            Times::Table.to_string(),
            Words::Table.to_string(),
            UserSelect::Table.to_string(),
            DishesImages::Table.to_string(),
            Dishes::Table.to_string(),
            Category::Table.to_string(),
            User::Table.to_string(),
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(&table)).to_owned())
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    Name,
    Password,
    CreateTime,
    Salt,
}

#[derive(DeriveIden)]
enum Category {
    Table,
    Id,
    Name,
    Desc,
}

#[derive(DeriveIden)]
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
    ViewId,
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

#[derive(DeriveIden)]
enum UserSelect {
    Table,
    Id,
    UserId,
    DishId,
    CreateTime,
}

#[derive(DeriveIden)]
enum Words {
    Table,
    Id,
    Word,
    Translation,
    CreateUser,
    Type,
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
