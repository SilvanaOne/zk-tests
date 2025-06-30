use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AgentFinishedJobEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentFinishedJobEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AgentFinishedJobEvent::CoordinatorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentFinishedJobEvent::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentFinishedJobEvent::EventData)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentFinishedJobEvent::CreatedAt)
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
                    .name("idx-agent_finished_job_events-coordinator_id")
                    .table(AgentFinishedJobEvent::Table)
                    .col(AgentFinishedJobEvent::CoordinatorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-agent_finished_job_events-timestamp")
                    .table(AgentFinishedJobEvent::Table)
                    .col(AgentFinishedJobEvent::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentFinishedJobEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AgentFinishedJobEvent {
    Table,
    Id,
    CoordinatorId,
    Timestamp,
    EventData,
    CreatedAt,
}
