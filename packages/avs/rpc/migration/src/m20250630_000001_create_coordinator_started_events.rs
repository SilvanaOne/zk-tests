use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CoordinatorStartedEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CoordinatorStartedEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorStartedEvent::CoordinatorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorStartedEvent::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorStartedEvent::EventData)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorStartedEvent::CreatedAt)
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
                    .name("idx-coordinator_started_events-coordinator_id")
                    .table(CoordinatorStartedEvent::Table)
                    .col(CoordinatorStartedEvent::CoordinatorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-coordinator_started_events-timestamp")
                    .table(CoordinatorStartedEvent::Table)
                    .col(CoordinatorStartedEvent::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CoordinatorStartedEvent::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CoordinatorStartedEvent {
    Table,
    Id,
    CoordinatorId,
    Timestamp,
    EventData,
    CreatedAt,
}
