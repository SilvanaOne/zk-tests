use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AgentTransactionEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentTransactionEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AgentTransactionEvent::CoordinatorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentTransactionEvent::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentTransactionEvent::EventData)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentTransactionEvent::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for efficient queries
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-agent_transaction_events-coordinator_id")
                    .table(AgentTransactionEvent::Table)
                    .col(AgentTransactionEvent::CoordinatorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-agent_transaction_events-timestamp")
                    .table(AgentTransactionEvent::Table)
                    .col(AgentTransactionEvent::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentTransactionEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AgentTransactionEvent {
    Table,
    Id,
    CoordinatorId,
    Timestamp,
    EventData,
    CreatedAt,
}
