use lambda_runtime::{Error, LambdaEvent};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct IncomingMessage {
    command: String,
}

#[derive(Serialize)]
pub struct OutgoingMessage {
    msg: String,
    error: Option<String>,
}

pub async fn function_handler(
    event: LambdaEvent<IncomingMessage>,
) -> Result<OutgoingMessage, Error> {
    let command = event.payload.command;

    let resp = OutgoingMessage {
        msg: format!("Command {}.", command),
        error: None,
    };

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_runtime::Context;

    #[tokio::test]
    async fn test_handler() {
        let event = LambdaEvent::new(
            IncomingMessage {
                command: "test".to_string(),
            },
            Context::default(),
        );
        let response = function_handler(event).await.unwrap();
        assert_eq!(response.msg, "Command test.");
    }
}
