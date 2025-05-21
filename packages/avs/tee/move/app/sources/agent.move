// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module app::agent;

use enclave::enclave::{Self, Enclave};
use std::string::String;

/// ====
/// Core onchain app logic, replace it with your own.
/// ====
///

const START_INTENT: u8 = 0;
const EInvalidSignature: u64 = 1;

public struct AgentNFT has key, store {
    id: UID,
    agent: String,
    action: u64,
    timestamp_ms: u64,
}

/// Should match the inner struct T used for IntentMessage<T> in Rust.
public struct AgentResponse has copy, drop {
    agent: String,
    action: u64,
}

public struct AGENT has drop {}

fun init(otw: AGENT, ctx: &mut TxContext) {
    let cap = enclave::new_cap(otw, ctx);

    cap.create_enclave_config(
        b"agent enclave".to_string(),
        x"000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", // pcr0
        x"000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", // pcr1
        x"000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", // pcr2
        ctx,
    );

    transfer::public_transfer(cap, ctx.sender())
}

public fun update_agent<T>(
    agent: String,
    action: u64,
    timestamp_ms: u64,
    sig: &vector<u8>,
    enclave: &Enclave<T>,
    ctx: &mut TxContext,
): AgentNFT {
    let res = enclave.verify_signature(
        START_INTENT,
        timestamp_ms,
        AgentResponse { agent, action },
        sig,
    );
    assert!(res, EInvalidSignature);
    // Mint NFT, replace it with your own logic.
    AgentNFT {
        id: object::new(ctx),
        agent,
        action,
        timestamp_ms,
    }
}
