use lambda_runtime::{Error, LambdaEvent};
use serde::Serialize;
use serde_json::Value;
use api::{process_request_async, ErrorResponse};
use tracing::{info, debug, error};

#[derive(Serialize)]
#[serde(untagged)]
pub enum OutgoingMessage {
    // For API Gateway responses
    ApiGateway {
        #[serde(rename = "statusCode")]
        status_code: i32,
        headers: Value,
        body: String,
    },
    // For Function URL / direct invocation
    Direct {
        #[serde(rename = "statusCode")]
        status_code: i32,
        headers: Value,
        body: String,
    },
}

pub async fn function_handler(event: LambdaEvent<Value>) -> Result<OutgoingMessage, Error> {
    // Determine invocation type
    let is_api_gateway = event.payload.get("httpMethod").is_some();
    let is_function_url = event.payload.get("requestContext")
        .and_then(|rc| rc.get("apiId"))
        .and_then(|id| id.as_str())
        .map(|id| id.contains("lambda-url"))
        .unwrap_or(false);
    
    let _invocation_type = if is_api_gateway {
        "API Gateway"
    } else if is_function_url {
        "Function URL"
    } else {
        "Direct Invocation"
    };

    // Extract request ID
    let _request_id = event.payload.get("requestContext")
        .and_then(|rc| rc.get("requestId"))
        .and_then(|id| id.as_str())
        .unwrap_or(&event.context.request_id);

    // Extract path from the event
    let path = event.payload.get("rawPath")
        .or_else(|| event.payload.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("/");

    // Extract HTTP method if available
    let http_method = event.payload.get("httpMethod")
        .or_else(|| event.payload.get("requestContext")
            .and_then(|rc| rc.get("http"))
            .and_then(|http| http.get("method")))
        .and_then(|m| m.as_str())
        .unwrap_or("UNKNOWN");

    // Extract source IP if available
    let source_ip = event.payload.get("requestContext")
        .and_then(|rc| rc.get("identity"))
        .and_then(|id| id.get("sourceIp"))
        .or_else(|| event.payload.get("requestContext")
            .and_then(|rc| rc.get("http"))
            .and_then(|http| http.get("sourceIp")))
        .and_then(|ip| ip.as_str())
        .unwrap_or("unknown");

    info!("Incoming {} request to {} from {}", http_method, path, source_ip);

    // Parse the body
    let body_str = if let Some(body) = event.payload.get("body") {
        let body_str = body.as_str().unwrap_or("{}");
        
        // Check if the body is base64 encoded
        let is_base64 = event
            .payload
            .get("isBase64Encoded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        if is_base64 {
            debug!("Body is base64 encoded");
            // Decode base64
            use base64::{Engine as _, engine::general_purpose};
            let decoded = general_purpose::STANDARD
                .decode(body_str)
                .map_err(|e| {
                    error!("Failed to decode base64: {}", e);
                    Error::from(format!("Failed to decode base64: {}", e))
                })?;
            String::from_utf8(decoded)
                .map_err(|e| {
                    error!("Failed to convert to UTF-8: {}", e);
                    Error::from(format!("Failed to convert to UTF-8: {}", e))
                })?
        } else {
            body_str.to_string()
        }
    } else {
        // Direct invocation with the entire payload as body
        serde_json::to_string(&event.payload)?
    };

    debug!("Request body: {}", body_str);

    // Process the request using the API crate
    let response_result = process_request_async(path, &body_str).await;

    // Return appropriate response format
    match response_result {
        Ok(response_body) => {
            info!("Successfully processed {} request", path);
            Ok(OutgoingMessage::ApiGateway {
                status_code: 200,
                headers: serde_json::json!({
                    "Content-Type": "application/json",
                }),
                body: response_body,
            })
        },
        Err(err) => {
            let error_response: ErrorResponse = err.into();
            let status_code = match error_response.error.as_str() {
                "INVALID_INPUT" | "INVALID_OPERATION" => 400,
                "OVERFLOW" => 400,
                _ => 500,
            };
            
            error!("Request to {} failed with {}: {}", path, status_code, error_response.message);
            
            Ok(OutgoingMessage::ApiGateway {
                status_code,
                headers: serde_json::json!({
                    "Content-Type": "application/json",
                }),
                body: serde_json::to_string(&error_response)?,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_runtime::Context;

    #[tokio::test]
    async fn test_add_function_url() {
        let event = LambdaEvent::new(
            serde_json::json!({
                "rawPath": "/add",
                "body": "{\"a\": 2, \"b\": 3}",
                "requestContext": {
                    "apiId": "dhctq4vocgpmdbp5so7jfql26q0ubzms"
                }
            }),
            Context::default(),
        );
        let response = function_handler(event).await.unwrap();
        match response {
            OutgoingMessage::Direct { status_code, body, .. } | 
            OutgoingMessage::ApiGateway { status_code, body, .. } => {
                assert_eq!(status_code, 200);
                assert!(body.contains("5"));
                assert!(body.contains("add"));
            }
        }
    }

    #[tokio::test]
    async fn test_multiply_api_gateway() {
        let event = LambdaEvent::new(
            serde_json::json!({
                "body": "{\"a\": 10, \"b\": 20}",
                "path": "/multiply",
                "httpMethod": "POST",
                "headers": {}
            }),
            Context::default(),
        );
        let response = function_handler(event).await.unwrap();
        match response {
            OutgoingMessage::ApiGateway { status_code, body, .. } => {
                assert_eq!(status_code, 200);
                assert!(body.contains("200"));
                assert!(body.contains("multiply"));
            }
            _ => panic!("Expected ApiGateway response"),
        }
    }

    #[tokio::test]
    async fn test_invalid_path() {
        let event = LambdaEvent::new(
            serde_json::json!({
                "rawPath": "/invalid",
                "body": "{\"a\": 1, \"b\": 2}"
            }),
            Context::default(),
        );
        let response = function_handler(event).await.unwrap();
        match response {
            OutgoingMessage::ApiGateway { status_code, .. } => {
                assert_eq!(status_code, 400);
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_overflow_error() {
        let event = LambdaEvent::new(
            serde_json::json!({
                "rawPath": "/add",
                "body": format!("{{\"a\": {}, \"b\": 1}}", i64::MAX)
            }),
            Context::default(),
        );
        let response = function_handler(event).await.unwrap();
        match response {
            OutgoingMessage::ApiGateway { status_code, body, .. } => {
                assert_eq!(status_code, 400);
                assert!(body.contains("OVERFLOW"));
            }
            _ => panic!("Expected error response"),
        }
    }
}