use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AgentStartedJobEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentStartedJobEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AgentStartedJobEvent::CoordinatorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentStartedJobEvent::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentStartedJobEvent::EventData)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentStartedJobEvent::CreatedAt)
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
                    .name("idx-agent_started_job_events-coordinator_id")
                    .table(AgentStartedJobEvent::Table)
                    .col(AgentStartedJobEvent::CoordinatorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-agent_started_job_events-timestamp")
                    .table(AgentStartedJobEvent::Table)
                    .col(AgentStartedJobEvent::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentStartedJobEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AgentStartedJobEvent {
    Table,
    Id,
    CoordinatorId,
    Timestamp,
    EventData,
    CreatedAt,
}
