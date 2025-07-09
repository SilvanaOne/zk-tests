use aws_sdk_cloudwatchlogs::Client as LogsClient;
use aws_sdk_ecs::Client as EcsClient;
use aws_sdk_ecs::types::{
    AssignPublicIp, AwsVpcConfiguration, CapacityProviderStrategyItem, ContainerDefinition,
    CpuArchitecture, LogConfiguration, LogDriver, NetworkConfiguration, NetworkMode,
};

use std::time::{Duration, Instant};
use tokio::time;

pub struct FargateConfig {
    pub cluster_name: String,
    pub subnet_ids: Vec<String>,
    pub security_group_ids: Vec<String>,
    pub task_role_arn: String,
    pub execution_role_arn: String,
    pub log_group_name: String,
    pub cpu: String,    // e.g., "256", "512", "1024"
    pub memory: String, // e.g., "512", "1024", "2048"
}

/// Run a container in AWS Fargate
pub async fn run_container_fargate(
    ecs_client: &EcsClient,
    logs_client: &LogsClient,
    config: &FargateConfig,
    image_name: &str,
    key: &str,
    agent: &str,
    action: &str,
    timeout_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    println!("Starting Fargate container...");

    // Create task definition
    let task_def_start = Instant::now();
    let task_definition_arn =
        create_task_definition(ecs_client, config, image_name, key, agent, action).await?;
    let task_def_time = task_def_start.elapsed();

    println!(
        "Task definition created in {:?}: {}",
        task_def_time, task_definition_arn
    );

    // Run the task
    let task_run_start = Instant::now();
    let task_arn = run_task(ecs_client, config, &task_definition_arn).await?;
    let task_run_time = task_run_start.elapsed();

    let container_start_time = start_time.elapsed();
    println!("Task submission time: {:?}", task_run_time);
    println!("Total Fargate startup time: {:?}", container_start_time);
    println!("Task started with ARN: {}", task_arn);

    // Monitor task execution with timeout
    let monitor_start = Instant::now();
    println!(
        "Waiting for Fargate task to complete (max {} seconds)...",
        timeout_seconds
    );

    match time::timeout(
        Duration::from_secs(timeout_seconds),
        monitor_task(ecs_client, config, &task_arn),
    )
    .await
    {
        Ok(result) => {
            result?;
            let monitor_time = monitor_start.elapsed();
            println!("Fargate task completed successfully in {:?}", monitor_time);

            // Get task logs
            let logs_start = Instant::now();
            get_task_logs(logs_client, config, &task_arn).await?;
            let logs_time = logs_start.elapsed();
            println!("Log retrieval took: {:?}", logs_time);
        }
        Err(_) => {
            let timeout_time = monitor_start.elapsed();
            println!(
                "Fargate task took too long (>{} sec, actual: {:?}), stopping it...",
                timeout_seconds, timeout_time
            );
            stop_task(ecs_client, config, &task_arn).await?;
        }
    }

    let container_runtime = start_time.elapsed();
    println!("Total Fargate execution time: {:?}", container_runtime);

    Ok(())
}

/// Create a task definition for the container
async fn create_task_definition(
    ecs_client: &EcsClient,
    config: &FargateConfig,
    image_name: &str,
    key: &str,
    agent: &str,
    action: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let container_name = format!("app-container-{}", chrono::Utc::now().timestamp());

    // Create log configuration
    let log_config = LogConfiguration::builder()
        .log_driver(LogDriver::Awslogs)
        .options("awslogs-group", &config.log_group_name)
        .options(
            "awslogs-region",
            std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        )
        .options("awslogs-stream-prefix", "fargate")
        .build();

    // Create container definition
    let container_def = ContainerDefinition::builder()
        .name(&container_name)
        .image(image_name)
        .essential(true)
        .command("npm")
        .command("run")
        .command("start")
        .command(key)
        .command(agent)
        .command(action)
        .port_mappings(
            aws_sdk_ecs::types::PortMapping::builder()
                .container_port(6000)
                .protocol(aws_sdk_ecs::types::TransportProtocol::Tcp)
                .build(),
        )
        .log_configuration(log_config?)
        .build();

    // Create task definition
    let task_def_name = format!("agent-task-{}", chrono::Utc::now().timestamp());

    let response = ecs_client
        .register_task_definition()
        .family(&task_def_name)
        .network_mode(NetworkMode::Awsvpc)
        .cpu(&config.cpu)
        .memory(&config.memory)
        .execution_role_arn(&config.execution_role_arn)
        .task_role_arn(&config.task_role_arn)
        .runtime_platform(
            aws_sdk_ecs::types::RuntimePlatform::builder()
                .cpu_architecture(CpuArchitecture::Arm64)
                .operating_system_family(aws_sdk_ecs::types::OsFamily::Linux)
                .build(),
        )
        .container_definitions(container_def)
        .send()
        .await?;

    let task_definition_arn = response
        .task_definition()
        .and_then(|td| td.task_definition_arn())
        .ok_or("Failed to get task definition ARN")?
        .to_string();

    Ok(task_definition_arn)
}

/// Run the task in Fargate with FARGATE_SPOT fallback to FARGATE
async fn run_task(
    ecs_client: &EcsClient,
    config: &FargateConfig,
    task_definition_arn: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let vpc_config = AwsVpcConfiguration::builder()
        .set_subnets(Some(config.subnet_ids.clone()))
        .set_security_groups(Some(config.security_group_ids.clone()))
        .assign_public_ip(AssignPublicIp::Enabled)
        .build();

    let network_config = NetworkConfiguration::builder()
        .awsvpc_configuration(vpc_config?)
        .build();

    // First try FARGATE_SPOT for cost savings
    println!("Attempting to start task with FARGATE_SPOT...");
    match try_run_task_with_capacity_provider(
        ecs_client,
        config,
        task_definition_arn,
        &network_config,
        "FARGATE_SPOT",
    )
    .await
    {
        Ok(task_arn) => {
            println!("✅ Task started successfully with FARGATE_SPOT (cost-optimized)");
            Ok(task_arn)
        }
        Err(e) => {
            println!("⚠️  FARGATE_SPOT failed: {}", e);
            println!("Falling back to FARGATE (standard pricing)...");

            // Fallback to regular FARGATE
            match try_run_task_with_capacity_provider(
                ecs_client,
                config,
                task_definition_arn,
                &network_config,
                "FARGATE",
            )
            .await
            {
                Ok(task_arn) => {
                    println!("✅ Task started successfully with FARGATE (standard pricing)");
                    Ok(task_arn)
                }
                Err(e) => {
                    println!("❌ Both FARGATE_SPOT and FARGATE failed");
                    Err(format!("Failed to start task with both capacity providers: {}", e).into())
                }
            }
        }
    }
}

/// Try to run a task with a specific capacity provider
async fn try_run_task_with_capacity_provider(
    ecs_client: &EcsClient,
    config: &FargateConfig,
    task_definition_arn: &str,
    network_config: &NetworkConfiguration,
    capacity_provider: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let capacity_provider_strategy = CapacityProviderStrategyItem::builder()
        .capacity_provider(capacity_provider)
        .weight(1)
        .build()?;

    let response = ecs_client
        .run_task()
        .cluster(&config.cluster_name)
        .task_definition(task_definition_arn)
        .capacity_provider_strategy(capacity_provider_strategy)
        .network_configuration(network_config.clone())
        .count(1)
        .send()
        .await?;

    // Check if task was actually created
    if response.tasks().is_empty() {
        return Err("No tasks were created - likely insufficient capacity".into());
    }

    // Check for failures in the response
    if !response.failures().is_empty() {
        let failures: Vec<String> = response
            .failures()
            .iter()
            .map(|f| {
                format!(
                    "ARN: {}, Reason: {}",
                    f.arn().unwrap_or("unknown"),
                    f.reason().unwrap_or("unknown")
                )
            })
            .collect();
        return Err(format!("Task creation failed: {}", failures.join(", ")).into());
    }

    let task_arn = response
        .tasks()
        .first()
        .and_then(|task| task.task_arn())
        .ok_or("Failed to get task ARN")?
        .to_string();

    Ok(task_arn)
}

/// Monitor task until it completes
async fn monitor_task(
    ecs_client: &EcsClient,
    config: &FargateConfig,
    task_arn: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_status = String::new();
    let mut status_start_time = Instant::now();
    let monitor_start = Instant::now();

    loop {
        let response = ecs_client
            .describe_tasks()
            .cluster(&config.cluster_name)
            .tasks(task_arn)
            .send()
            .await?;

        if let Some(task) = response.tasks().first() {
            if let Some(current_status) = task.last_status() {
                // If status changed, print timing for previous status
                if current_status != last_status && !last_status.is_empty() {
                    let status_duration = status_start_time.elapsed();
                    println!("Status '{}' took: {:?}", last_status, status_duration);
                    status_start_time = Instant::now();
                }

                // If this is a new status, print it
                if current_status != last_status {
                    let total_elapsed = monitor_start.elapsed();
                    println!(
                        "Task status: {} (after {:?})",
                        current_status, total_elapsed
                    );
                    last_status = current_status.to_string();
                }

                match current_status {
                    "STOPPED" => {
                        let final_status_duration = status_start_time.elapsed();
                        println!(
                            "Final status '{}' took: {:?}",
                            current_status, final_status_duration
                        );

                        // Check exit code and reasons
                        let containers = task.containers();
                        for container in containers {
                            if let Some(exit_code) = container.exit_code() {
                                println!("Container exited with code: {}", exit_code);
                            }
                            if let Some(reason) = container.reason() {
                                println!("Container stop reason: {}", reason);
                            }
                        }
                        break;
                    }
                    "RUNNING" => {
                        if current_status != last_status {
                            println!("🚀 Task is now running!");
                        }
                    }
                    _ => {
                        // Other statuses like PROVISIONING, PENDING, DEPROVISIONING
                        // Just track them, timing is handled above
                    }
                }
            }
        }

        time::sleep(Duration::from_secs(1)).await;
    }

    let total_monitor_time = monitor_start.elapsed();
    println!("Total monitoring time: {:?}", total_monitor_time);
    Ok(())
}

/// Stop a running task
async fn stop_task(
    ecs_client: &EcsClient,
    config: &FargateConfig,
    task_arn: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    ecs_client
        .stop_task()
        .cluster(&config.cluster_name)
        .task(task_arn)
        .reason("Timeout exceeded")
        .send()
        .await?;

    println!("Task stopped");
    Ok(())
}

/// Get logs from CloudWatch for the task
async fn get_task_logs(
    logs_client: &LogsClient,
    config: &FargateConfig,
    task_arn: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let logs_start = Instant::now();

    // Extract task ID from ARN
    let task_id = task_arn.split('/').last().ok_or("Invalid task ARN")?;

    // Wait a bit for logs to be available
    println!("Waiting 2 seconds for logs to become available...");
    time::sleep(Duration::from_secs(2)).await;

    // Find the correct log stream by searching for streams that contain the task ID
    let list_streams_start = Instant::now();

    match logs_client
        .describe_log_streams()
        .log_group_name(&config.log_group_name)
        .order_by(aws_sdk_cloudwatchlogs::types::OrderBy::LastEventTime)
        .descending(true)
        .limit(50) // Get the most recent 50 streams
        .send()
        .await
    {
        Ok(streams_response) => {
            let list_streams_time = list_streams_start.elapsed();
            println!("Listed log streams in: {:?}", list_streams_time);

            let streams = streams_response.log_streams();
            let mut found_stream = None;

            // Look for a stream that contains our task ID
            for stream in streams {
                if let Some(stream_name) = stream.log_stream_name() {
                    if stream_name.contains(task_id) {
                        found_stream = Some(stream_name);
                        break;
                    }
                }
            }

            if let Some(log_stream_name) = found_stream {
                println!("Found log stream: {}", log_stream_name);

                let api_call_start = Instant::now();
                match logs_client
                    .get_log_events()
                    .log_group_name(&config.log_group_name)
                    .log_stream_name(log_stream_name)
                    .start_from_head(true)
                    .send()
                    .await
                {
                    Ok(response) => {
                        let api_call_time = api_call_start.elapsed();
                        println!("CloudWatch API call took: {:?}", api_call_time);

                        let events = response.events();
                        let event_count = events.len();

                        if event_count > 0 {
                            println!("Found {} log events:", event_count);
                            for (i, event) in events.iter().enumerate() {
                                if let Some(message) = event.message() {
                                    println!("[{}] {}", i + 1, message);
                                }
                            }
                        } else {
                            println!("No log events found in the stream");
                        }
                    }
                    Err(e) => {
                        let api_call_time = api_call_start.elapsed();
                        println!(
                            "CloudWatch API call failed after {:?}: {}",
                            api_call_time, e
                        );
                    }
                }
            } else {
                println!("No log stream found for task ID: {}", task_id);
                println!("Available streams:");
                for stream in streams.iter().take(10) {
                    if let Some(stream_name) = stream.log_stream_name() {
                        println!("  - {}", stream_name);
                    }
                }
            }
        }
        Err(e) => {
            let list_streams_time = list_streams_start.elapsed();
            println!(
                "Failed to list log streams after {:?}: {}",
                list_streams_time, e
            );
        }
    }

    let total_logs_time = logs_start.elapsed();
    println!("Total log retrieval time: {:?}", total_logs_time);
    Ok(())
}

/// Check if the required image exists in ECR or Docker Hub
pub async fn check_image_exists(image_name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // For now, assume the image exists if it follows the expected pattern
    // In a real implementation, you would check ECR or Docker Hub API
    println!("Checking if image exists: {}", image_name);

    if image_name.starts_with("dfstio/") {
        println!("Image assumed to exist in Docker Hub");
        Ok(true)
    } else {
        println!("Image pattern not recognized");
        Ok(false)
    }
}

/// Load Fargate configuration from environment variables
/// These variables are generated by the Pulumi script and saved to .env.pulumi
///
/// Example usage in agent.rs:
/// ```rust
/// use crate::fargate::{run_container_fargate, load_fargate_config_from_env};
/// use aws_config;
/// use aws_sdk_ecs::Client as EcsClient;
/// use aws_sdk_cloudwatchlogs::Client as LogsClient;
///
/// // Load AWS configuration
/// let config = aws_config::load_from_env().await;
/// let ecs_client = EcsClient::new(&config);
/// let logs_client = LogsClient::new(&config);
///
/// // Load Fargate configuration from environment variables
/// let fargate_config = load_fargate_config_from_env()?;
///
/// // Run container in Fargate instead of local Docker
/// run_container_fargate(
///     &ecs_client,
///     &logs_client,
///     &fargate_config,
///     &image_name,
///     &key,
///     &request.agent,
///     &request.action,
///     900,
/// ).await?;
/// ```
pub fn load_fargate_config_from_env() -> Result<FargateConfig, Box<dyn std::error::Error>> {
    let cluster_name = std::env::var("FARGATE_CLUSTER_NAME")
        .map_err(|_| "FARGATE_CLUSTER_NAME environment variable not set")?;

    let subnet_ids = std::env::var("FARGATE_SUBNET_IDS")
        .map_err(|_| "FARGATE_SUBNET_IDS environment variable not set")?
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let security_group_ids = std::env::var("FARGATE_SECURITY_GROUP_IDS")
        .map_err(|_| "FARGATE_SECURITY_GROUP_IDS environment variable not set")?
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let task_role_arn = std::env::var("FARGATE_TASK_ROLE_ARN")
        .map_err(|_| "FARGATE_TASK_ROLE_ARN environment variable not set")?;

    let execution_role_arn = std::env::var("FARGATE_EXECUTION_ROLE_ARN")
        .map_err(|_| "FARGATE_EXECUTION_ROLE_ARN environment variable not set")?;

    let log_group_name = std::env::var("FARGATE_LOG_GROUP_NAME")
        .map_err(|_| "FARGATE_LOG_GROUP_NAME environment variable not set")?;

    let cpu = std::env::var("FARGATE_CPU").unwrap_or_else(|_| "512".to_string());
    let memory = std::env::var("FARGATE_MEMORY").unwrap_or_else(|_| "1024".to_string());

    Ok(FargateConfig {
        cluster_name,
        subnet_ids,
        security_group_ids,
        task_role_arn,
        execution_role_arn,
        log_group_name,
        cpu,
        memory,
    })
}
