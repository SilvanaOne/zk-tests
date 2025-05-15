module coordination::agent;

use std::string::String;

public struct Agent has key, store {
    id: UID,
    name: String,
    action: String,
}

fun init(ctx: &mut TxContext) {
    let agent = Agent {
        id: object::new(ctx),
        name: b"empty".to_string(),
        action: b"empty".to_string(),
    };
    transfer::share_object(agent)
}

public fun run_agent(agent: &mut Agent, name: String, action: String) {
    agent.name = name;
    agent.action = action;
}
