use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CoordinatorErrorEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CoordinatorErrorEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorErrorEvent::CoordinatorId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorErrorEvent::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorErrorEvent::EventData)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CoordinatorErrorEvent::CreatedAt)
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
                    .name("idx-coordinator_error_events-coordinator_id")
                    .table(CoordinatorErrorEvent::Table)
                    .col(CoordinatorErrorEvent::CoordinatorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-coordinator_error_events-timestamp")
                    .table(CoordinatorErrorEvent::Table)
                    .col(CoordinatorErrorEvent::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CoordinatorErrorEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CoordinatorErrorEvent {
    Table,
    Id,
    CoordinatorId,
    Timestamp,
    EventData,
    CreatedAt,
}
