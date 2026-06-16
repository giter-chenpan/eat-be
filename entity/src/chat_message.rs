//! `SeaORM` Entity for chat_messages

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "chat_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub error: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_args: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chat_session::Entity",
        from = "Column::SessionId",
        to = "super::chat_session::Column::Id"
    )]
    BelongsToChatSession,
}

impl Related<super::chat_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BelongsToChatSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
