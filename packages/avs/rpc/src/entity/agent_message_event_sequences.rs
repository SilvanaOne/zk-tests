//! Child entity for `sequences`. `AgentMessageEvent` -> `agent_message_event_sequences`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_message_event_sequences")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub agent_message_event_id: i64,
    pub sequence: i64,
    pub created_at: Option<DateTimeUtc>,
    pub updated_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
