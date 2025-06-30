use crate::events::*;
use sea_orm::ActiveValue::*;

pub fn coordinator_started_event(
    proto: CoordinatorStartedEvent,
) -> super::coordinator_started_event::ActiveModel {
    super::coordinator_started_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "ethereum_address:{},sui_address:{}",
            proto.ethereum_address, proto.sui_ed25519_address
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

// Placeholder implementations - these will be created for each event type
pub fn agent_started_job_event(
    proto: AgentStartedJobEvent,
) -> super::agent_started_job_event::ActiveModel {
    super::agent_started_job_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "developer:{},agent:{},app:{},job_id:{}",
            proto.developer, proto.agent, proto.app, proto.job_id
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn agent_finished_job_event(
    proto: AgentFinishedJobEvent,
) -> super::agent_finished_job_event::ActiveModel {
    super::agent_finished_job_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "developer:{},agent:{},app:{},job_id:{},duration:{}",
            proto.developer, proto.agent, proto.app, proto.job_id, proto.duration
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn coordination_tx_event(
    proto: CoordinationTxEvent,
) -> super::coordination_tx_event::ActiveModel {
    super::coordination_tx_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "developer:{},agent:{},app:{},job_id:{},memo:{},tx_hash:{}",
            proto.developer, proto.agent, proto.app, proto.job_id, proto.memo, proto.tx_hash
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn coordinator_error_event(
    proto: CoordinatorErrorEvent,
) -> super::coordinator_error_event::ActiveModel {
    super::coordinator_error_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!("error:{}", proto.error)),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn client_transaction_event(
    proto: ClientTransactionEvent,
) -> super::client_transaction_event::ActiveModel {
    super::client_transaction_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "developer:{},agent:{},app:{},client_ip:{},method:{},tx_hash:{},sequence:{}",
            proto.developer,
            proto.agent,
            proto.app,
            proto.client_ip_address,
            proto.method,
            proto.tx_hash,
            proto.sequence
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn agent_message_event(proto: AgentMessageEvent) -> super::agent_message_event::ActiveModel {
    super::agent_message_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "type:{},developer:{},agent:{},app:{},job_id:{},message:{}",
            proto.r#type, proto.developer, proto.agent, proto.app, proto.job_id, proto.message
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn agent_error_event(proto: AgentErrorEvent) -> super::agent_error_event::ActiveModel {
    super::agent_error_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!(
            "type:{},developer:{},agent:{},app:{},job_id:{},error:{}",
            proto.r#type, proto.developer, proto.agent, proto.app, proto.job_id, proto.error
        )),
        created_at: Set(chrono::Utc::now()),
    }
}

pub fn agent_transaction_event(
    proto: AgentTransactionEvent,
) -> super::agent_transaction_event::ActiveModel {
    super::agent_transaction_event::ActiveModel {
        id: NotSet,
        coordinator_id: Set(proto.coordinator_id),
        timestamp: Set(proto.timestamp as i64),
        event_data: Set(format!("type:{},developer:{},agent:{},app:{},job_id:{},tx_hash:{},chain:{},network:{},tx_type:{},memo:{}", 
            proto.r#type, proto.developer, proto.agent, proto.app, proto.job_id, proto.tx_hash, proto.chain, proto.network, proto.tx_type, proto.memo)),
        created_at: Set(chrono::Utc::now()),
    }
}
