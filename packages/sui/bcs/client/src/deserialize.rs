use move_core_types::u256::U256;
use serde::{Deserialize, Serialize};

/// UserStateData struct that mirrors the Move struct in sources/bcs.move
/// This matches the structure of UserStateData in Move:
/// - name: String
/// - data: u256
/// - signature: vector<u8>
/// - sequence: u64
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UserStateData {
    pub name: String,
    pub data: U256,
    pub signature: Vec<u8>,
    pub sequence: u64,
}

/// UserStateEvent struct that mirrors the Move event in sources/bcs.move
/// This matches the structure of UserStateEvent in Move:
/// - name: String
/// - data: u256
/// - signature: vector<u8>
/// - sequence: u64
/// - serialized_state: vector<u8>
#[derive(Debug, Clone)]
pub struct UserStateEvent {
    pub name: String,
    pub data: U256,
    pub signature: Vec<u8>,
    pub sequence: u64,
    pub serialized_state: Vec<u8>,
}

impl UserStateEvent {
    /// Create UserStateEvent from JSON event data returned by Move
    pub fn from_json_event(json: &serde_json::Value) -> Result<Self, anyhow::Error> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'name' field"))?
            .to_string();

        let data_str = json
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'data' field"))?;
        let data = U256::from_str_radix(data_str, 10)
            .map_err(|e| anyhow::anyhow!("Failed to parse data: {}", e))?;

        let signature = json
            .get("signature")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'signature' field"))?
            .iter()
            .map(|v| v.as_u64().map(|n| n as u8))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| anyhow::anyhow!("Invalid signature array"))?;

        let sequence = json
            .get("sequence")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'sequence' field"))?;

        // Parse the serialized_state from base64 or array format
        let serialized_state =
            if let Some(state_array) = json.get("serialized_state").and_then(|v| v.as_array()) {
                // If it's an array of numbers
                println!("serialized_state is an array of numbers");
                state_array
                    .iter()
                    .map(|v| v.as_u64().map(|n| n as u8))
                    .collect::<Option<Vec<u8>>>()
                    .ok_or_else(|| anyhow::anyhow!("Invalid serialized_state array"))?
            } else if let Some(state_str) = json.get("serialized_state").and_then(|v| v.as_str()) {
                // If it's a base64 string
                println!("serialized_state is a base64 string");
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                STANDARD.decode(state_str).map_err(|e| {
                    anyhow::anyhow!("Failed to decode base64 serialized_state: {}", e)
                })?
            } else {
                return Err(anyhow::anyhow!(
                    "Missing or invalid 'serialized_state' field"
                ));
            };

        Ok(UserStateEvent {
            name,
            data,
            signature,
            sequence,
            serialized_state,
        })
    }

    /// Deserialize the serialized_state field from BCS bytes into UserStateData
    pub fn deserialize_state(&self) -> Result<UserStateData, bcs::Error> {
        bcs::from_bytes::<UserStateData>(&self.serialized_state)
    }

    /// Verify that the deserialized state matches the event parameters
    pub fn verify_state_consistency(&self) -> Result<bool, bcs::Error> {
        let deserialized = self.deserialize_state()?;

        Ok(deserialized.name == self.name
            && deserialized.data == self.data
            && deserialized.signature == self.signature
            && deserialized.sequence == self.sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_userstate_data_serialization() {
        let user_state_data = UserStateData {
            name: "Alice".to_string(),
            data: U256::from(42u64),
            signature: vec![1, 2, 3, 4],
            sequence: 1,
        };

        // Serialize
        let serialized = bcs::to_bytes(&user_state_data).expect("Failed to serialize");
        println!(
            "Serialized UserStateData ({} bytes): {:02x?}",
            serialized.len(),
            serialized
        );

        // Deserialize
        let deserialized: UserStateData =
            bcs::from_bytes(&serialized).expect("Failed to deserialize");

        // Verify
        assert_eq!(deserialized, user_state_data);
        assert_eq!(deserialized.name, "Alice");
        assert_eq!(deserialized.data, U256::from(42u64));
        assert_eq!(deserialized.signature, vec![1, 2, 3, 4]);
        assert_eq!(deserialized.sequence, 1);
    }

    #[test]
    fn test_event_deserialization() {
        // Create a UserStateData
        let user_state_data = UserStateData {
            name: "Bob".to_string(),
            data: U256::from(100u64),
            signature: vec![5, 6, 7, 8],
            sequence: 2,
        };

        // Serialize it
        let serialized_state = bcs::to_bytes(&user_state_data).expect("Failed to serialize");

        // Create an event with the serialized state
        let event = UserStateEvent {
            name: "Bob".to_string(),
            data: U256::from(100u64),
            signature: vec![5, 6, 7, 8],
            sequence: 2,
            serialized_state,
        };

        // Deserialize and verify
        let deserialized = event.deserialize_state().expect("Failed to deserialize");
        assert_eq!(deserialized, user_state_data);

        // Verify consistency
        assert!(
            event
                .verify_state_consistency()
                .expect("Failed to verify consistency")
        );
    }

    #[test]
    fn test_consistency_check_fails_on_mismatch() {
        // Create a UserStateData with different values
        let user_state_data = UserStateData {
            name: "Charlie".to_string(),
            data: U256::from(200u64),
            signature: vec![9, 10, 11, 12],
            sequence: 3,
        };

        // Serialize it
        let serialized_state = bcs::to_bytes(&user_state_data).expect("Failed to serialize");

        // Create an event with DIFFERENT parameters (mismatch)
        let event = UserStateEvent {
            name: "Different".to_string(), // Different name
            data: U256::from(100u64),      // Different data
            signature: vec![5, 6, 7, 8],   // Different signature
            sequence: 2,                   // Different sequence
            serialized_state,
        };

        // Verify consistency should fail
        assert!(
            !event
                .verify_state_consistency()
                .expect("Failed to verify consistency")
        );
    }
}
