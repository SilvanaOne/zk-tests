module coordination::agent;

use std::string::String;
use sui::event;

public struct AgentRequest has key, store {
    id: UID,
    nonce: u64,
    name: String,
    action: String,
    request: String,
}

public struct AgentRequestEvent has copy, drop {
    nonce: u64,
    name: String,
    action: String,
    request: String,
}

public struct AgentResponse has key, store {
    id: UID,
    nonce: u64,
    name: String,
    action: String,
    result: String,
}

public struct AgentResponseEvent has copy, drop {
    nonce: u64,
    name: String,
    action: String,
    result: String,
}

fun init(ctx: &mut TxContext) {
    let agent_request = AgentRequest {
        id: object::new(ctx),
        nonce: 0,
        name: b"AgentRequest".to_string(),
        action: b"empty".to_string(),
        request: b"empty".to_string(),
    };
    transfer::share_object(agent_request);

    let agent_response = AgentResponse {
        id: object::new(ctx),
        nonce: 0,
        name: b"AgentResponse".to_string(),
        action: b"empty".to_string(),
        result: b"empty".to_string(),
    };
    transfer::share_object(agent_response);
}

public fun agent_request(
    agent_request: &mut AgentRequest,
    name: String,
    action: String,
    request: String,
) {
    agent_request.name = name;
    agent_request.action = action;
    agent_request.request = request;
    agent_request.nonce = agent_request.nonce + 1;
    event::emit(AgentRequestEvent {
        nonce: agent_request.nonce,
        name: name,
        action: action,
        request: request,
    });
}

public fun agent_response(
    agent_response: &mut AgentResponse,
    name: String,
    action: String,
    result: String,
) {
    agent_response.name = name;
    agent_response.action = action;
    agent_response.result = result;
    agent_response.nonce = agent_response.nonce + 1;
    event::emit(AgentResponseEvent {
        nonce: agent_response.nonce,
        name: name,
        action: action,
        result: result,
    });
}
