#!/bin/bash

# Create remaining migration files with basic structure
# These will need to be customized based on the specific fields

migrations=(
    "m20240101_000002_create_agent_started_job_events.rs:AgentStartedJobEvent"
    "m20240101_000003_create_agent_finished_job_events.rs:AgentFinishedJobEvent"
    "m20240101_000004_create_coordination_tx_events.rs:CoordinationTxEvent"
    "m20240101_000005_create_coordinator_error_events.rs:CoordinatorErrorEvent"
    "m20240101_000006_create_client_transaction_events.rs:ClientTransactionEvent"
    "m20240101_000008_create_agent_error_events.rs:AgentErrorEvent"
    "m20240101_000009_create_agent_transaction_events.rs:AgentTransactionEvent"
)

for migration in "${migrations[@]}"; do
    IFS=':' read -r filename table_name <<< "$migration"
    cat > "migration/src/$filename" << MIGRATION_EOF
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(${table_name}::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(${table_name}::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // TODO: Add specific columns based on protobuf definition
                    .col(
                        ColumnDef::new(${table_name}::Timestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(${table_name}::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-${table_name,,}-timestamp")
                    .table(${table_name}::Table)
                    .col(${table_name}::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(${table_name}::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ${table_name} {
    Table,
    Id,
    Timestamp,
    CreatedAt,
}
MIGRATION_EOF
    echo "Created $filename"
done
