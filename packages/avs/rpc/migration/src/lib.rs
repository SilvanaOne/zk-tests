pub use sea_orm_migration::prelude::*;

mod m20250630_000001_create_coordinator_started_events;
mod m20250630_000002_create_agent_started_job_events;
mod m20250630_000003_create_agent_finished_job_events;
mod m20250630_000004_create_coordination_tx_events;
mod m20250630_000005_create_coordinator_error_events;
mod m20250630_000006_create_client_transaction_events;
mod m20250630_000007_create_agent_message_events;
mod m20250630_000008_create_agent_error_events;
mod m20250630_000009_create_agent_transaction_events;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250630_000001_create_coordinator_started_events::Migration),
            Box::new(m20250630_000002_create_agent_started_job_events::Migration),
            Box::new(m20250630_000003_create_agent_finished_job_events::Migration),
            Box::new(m20250630_000004_create_coordination_tx_events::Migration),
            Box::new(m20250630_000005_create_coordinator_error_events::Migration),
            Box::new(m20250630_000006_create_client_transaction_events::Migration),
            Box::new(m20250630_000007_create_agent_message_events::Migration),
            Box::new(m20250630_000008_create_agent_error_events::Migration),
            Box::new(m20250630_000009_create_agent_transaction_events::Migration),
        ]
    }
}
