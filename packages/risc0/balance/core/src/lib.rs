use risc0_zkvm::serde::to_vec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Copy)]
pub struct State {
    pub sequence: u64,
    pub balance: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Copy)]
pub struct StepOperation {
    pub add: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Copy)]
pub struct StepRequest {
    pub state: State,
    pub operation: StepOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Copy)]
pub struct StepPublicData {
    pub input: State,
    pub output: State,
}

impl StepPublicData {
    pub fn to_vec(&self) -> Vec<u32> {
        to_vec(self).unwrap()
    }
}
