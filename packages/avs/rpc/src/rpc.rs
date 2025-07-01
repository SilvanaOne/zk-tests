use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, warn};

use crate::buffer::EventBuffer;
use crate::database::EventDatabase;
use crate::events::{
    silvana_events_service_server::SilvanaEventsService, AgentMessageEventWithId,
    AgentTransactionEventWithId, Event, GetAgentMessageEventsBySequenceRequest,
    GetAgentMessageEventsBySequenceResponse, GetAgentTransactionEventsBySequenceRequest,
    GetAgentTransactionEventsBySequenceResponse, SubmitEventsRequest, SubmitEventsResponse,
};

pub struct SilvanaEventsServiceImpl {
    event_buffer: EventBuffer,
    database: Arc<EventDatabase>,
}

impl SilvanaEventsServiceImpl {
    pub fn new(event_buffer: EventBuffer, database: Arc<EventDatabase>) -> Self {
        Self {
            event_buffer,
            database,
        }
    }
}

#[tonic::async_trait]
impl SilvanaEventsService for SilvanaEventsServiceImpl {
    async fn submit_events(
        &self,
        request: Request<SubmitEventsRequest>,
    ) -> Result<Response<SubmitEventsResponse>, Status> {
        let events = request.into_inner().events;
        let event_count = events.len();

        debug!("Received batch of {} events", event_count);

        let mut processed_count = 0;
        let mut first_error: Option<String> = None;

        for event in events {
            match self.event_buffer.add_event(event).await {
                Ok(()) => processed_count += 1,
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(e.to_string());
                    }
                    // Continue processing other events even if one fails
                }
            }
        }

        let success = processed_count == event_count;
        let message = if success {
            format!("Successfully queued {} events", processed_count)
        } else {
            format!(
                "Queued {}/{} events. First error: {}",
                processed_count,
                event_count,
                first_error.unwrap_or_else(|| "Unknown error".to_string())
            )
        };

        if !success {
            warn!("{}", message);
        }

        Ok(Response::new(SubmitEventsResponse {
            success,
            message,
            processed_count: processed_count as u32,
        }))
    }

    async fn submit_event(
        &self,
        request: Request<Event>,
    ) -> Result<Response<SubmitEventsResponse>, Status> {
        let event = request.into_inner();

        debug!("Received single event");

        match self.event_buffer.add_event(event).await {
            Ok(()) => Ok(Response::new(SubmitEventsResponse {
                success: true,
                message: "Event queued successfully".to_string(),
                processed_count: 1,
            })),
            Err(e) => {
                warn!("Failed to queue event: {}", e);

                // Return appropriate error based on the failure type
                let status =
                    if e.to_string().contains("overloaded") || e.to_string().contains("timeout") {
                        Status::resource_exhausted(e.to_string())
                    } else if e.to_string().contains("Memory limit") {
                        Status::resource_exhausted(e.to_string())
                    } else if e.to_string().contains("Circuit breaker") {
                        Status::unavailable(e.to_string())
                    } else {
                        Status::internal(e.to_string())
                    };

                Err(status)
            }
        }
    }

    async fn get_agent_transaction_events_by_sequence(
        &self,
        request: Request<GetAgentTransactionEventsBySequenceRequest>,
    ) -> Result<Response<GetAgentTransactionEventsBySequenceResponse>, Status> {
        let req = request.into_inner();

        debug!(
            "Querying agent transaction events by sequence: {}",
            req.sequence
        );

        match self
            .database
            .get_agent_transaction_events_by_sequence(
                req.sequence,
                req.limit,
                req.offset,
                req.coordinator_id,
                req.developer,
                req.agent,
                req.app,
            )
            .await
        {
            Ok((events, total_count)) => {
                let proto_events: Vec<AgentTransactionEventWithId> = events
                    .into_iter()
                    .map(|event| AgentTransactionEventWithId {
                        id: event.id,
                        coordinator_id: event.coordinator_id,
                        tx_type: event.tx_type,
                        developer: event.developer,
                        agent: event.agent,
                        app: event.app,
                        job_id: event.job_id,
                        sequences: event.sequences,
                        event_timestamp: event.event_timestamp,
                        tx_hash: event.tx_hash,
                        chain: event.chain,
                        network: event.network,
                        memo: event.memo,
                        metadata: event.metadata,
                        created_at_timestamp: event.created_at_timestamp,
                    })
                    .collect();

                let returned_count = proto_events.len() as u32;

                debug!(
                    "Found {} agent transaction events for sequence {}",
                    returned_count, req.sequence
                );

                Ok(Response::new(GetAgentTransactionEventsBySequenceResponse {
                    success: true,
                    message: format!("Found {} events", returned_count),
                    events: proto_events,
                    total_count,
                    returned_count,
                }))
            }
            Err(e) => {
                error!(
                    "Failed to query agent transaction events by sequence: {}",
                    e
                );
                Err(Status::internal(format!("Database query failed: {}", e)))
            }
        }
    }

    async fn get_agent_message_events_by_sequence(
        &self,
        request: Request<GetAgentMessageEventsBySequenceRequest>,
    ) -> Result<Response<GetAgentMessageEventsBySequenceResponse>, Status> {
        let req = request.into_inner();

        debug!(
            "Querying agent message events by sequence: {}",
            req.sequence
        );

        match self
            .database
            .get_agent_message_events_by_sequence(
                req.sequence,
                req.limit,
                req.offset,
                req.coordinator_id,
                req.developer,
                req.agent,
                req.app,
            )
            .await
        {
            Ok((events, total_count)) => {
                let proto_events: Vec<AgentMessageEventWithId> = events
                    .into_iter()
                    .map(|event| {
                        AgentMessageEventWithId {
                            id: event.id,
                            coordinator_id: event.coordinator_id,
                            developer: event.developer,
                            agent: event.agent,
                            app: event.app,
                            job_id: event.job_id,
                            sequences: event.sequences,
                            event_timestamp: event.event_timestamp,
                            level: event.level as i32, // Convert to protobuf enum
                            message: event.message,
                            created_at_timestamp: event.created_at_timestamp,
                        }
                    })
                    .collect();

                let returned_count = proto_events.len() as u32;

                debug!(
                    "Found {} agent message events for sequence {}",
                    returned_count, req.sequence
                );

                Ok(Response::new(GetAgentMessageEventsBySequenceResponse {
                    success: true,
                    message: format!("Found {} events", returned_count),
                    events: proto_events,
                    total_count,
                    returned_count,
                }))
            }
            Err(e) => {
                error!("Failed to query agent message events by sequence: {}", e);
                Err(Status::internal(format!("Database query failed: {}", e)))
            }
        }
    }
}
