use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChatSessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ChatSessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ChatSessions::UserId).string().not_null())
                    .col(ColumnDef::new(ChatSessions::Title).string().not_null())
                    .col(ColumnDef::new(ChatSessions::CreatedAt).string().not_null())
                    .col(ColumnDef::new(ChatSessions::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_chat_sessions_user_updated")
                    .table(ChatSessions::Table)
                    .col(ChatSessions::UserId)
                    .col(ChatSessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ChatMessages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ChatMessages::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ChatMessages::SessionId).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Role).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Content).text().not_null())
                    .col(ColumnDef::new(ChatMessages::Status).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Error).text().null())
                    .col(ColumnDef::new(ChatMessages::ToolCallId).string().null())
                    .col(ColumnDef::new(ChatMessages::ToolName).string().null())
                    .col(ColumnDef::new(ChatMessages::ToolArgs).text().null())
                    .col(ColumnDef::new(ChatMessages::PromptTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::CompletionTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::CreatedAt).string().not_null())
                    .foreign_key(
                        &mut ForeignKey::create()
                            .name("fk_chat_messages_session")
                            .from(ChatMessages::Table, ChatMessages::SessionId)
                            .to(ChatSessions::Table, ChatSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_chat_messages_session_created")
                    .table(ChatMessages::Table)
                    .col(ChatMessages::SessionId)
                    .col(ChatMessages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChatMessages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ChatSessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ChatSessions {
    Table,
    Id,
    UserId,
    Title,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ChatMessages {
    Table,
    Id,
    SessionId,
    Role,
    Content,
    Status,
    Error,
    ToolCallId,
    ToolName,
    ToolArgs,
    PromptTokens,
    CompletionTokens,
    CreatedAt,
}
