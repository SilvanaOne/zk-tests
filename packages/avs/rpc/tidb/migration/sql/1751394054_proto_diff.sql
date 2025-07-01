-- dry run --
CREATE TABLE IF NOT EXISTS coordinator_started_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `ethereum_address` VARCHAR(255) NOT NULL,
    `sui_ed_25519_address` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_started_job_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `job_id` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_job_id (`job_id`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_finished_job_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `job_id` VARCHAR(255) NOT NULL,
    `duration` BIGINT NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_job_id (`job_id`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS coordination_tx_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `job_id` VARCHAR(255) NOT NULL,
    `memo` VARCHAR(255) NOT NULL,
    `tx_hash` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_job_id (`job_id`),
    INDEX idx_tx_hash (`tx_hash`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS coordinator_message_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `level` JSON NOT NULL,
    `message` VARCHAR(255) NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS client_transaction_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `client_ip_address` VARCHAR(255) NOT NULL,
    `method` VARCHAR(255) NOT NULL,
    `data` BLOB NOT NULL,
    `tx_hash` VARCHAR(255) NOT NULL,
    `sequence` BIGINT NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_tx_hash (`tx_hash`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_message_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `job_id` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `level` JSON NOT NULL,
    `message` VARCHAR(255) NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_job_id (`job_id`),
    INDEX idx_event_timestamp (`event_timestamp`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_message_event_sequences (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `agent_message_event_id` BIGINT NOT NULL,
    `sequence` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_agent_message_event_sequences_parent (`agent_message_event_id`),
    INDEX idx_agent_message_event_sequences_value (`sequence`),
    FOREIGN KEY (`agent_message_event_id`) REFERENCES agent_message_event (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_transaction_event (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `coordinator_id` VARCHAR(255) NOT NULL,
    `tx_type` VARCHAR(255) NOT NULL,
    `developer` VARCHAR(255) NOT NULL,
    `agent` VARCHAR(255) NOT NULL,
    `app` VARCHAR(255) NOT NULL,
    `job_id` VARCHAR(255) NOT NULL,
    `event_timestamp` BIGINT NOT NULL,
    `tx_hash` VARCHAR(255) NOT NULL,
    `chain` VARCHAR(255) NOT NULL,
    `network` VARCHAR(255) NOT NULL,
    `memo` VARCHAR(255) NOT NULL,
    `metadata` VARCHAR(255) NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_created_at (`created_at`),
    INDEX idx_coordinator_id (`coordinator_id`),
    INDEX idx_job_id (`job_id`),
    INDEX idx_event_timestamp (`event_timestamp`),
    INDEX idx_tx_hash (`tx_hash`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
CREATE TABLE IF NOT EXISTS agent_transaction_event_sequences (
    `id` BIGINT AUTO_INCREMENT PRIMARY KEY,
    `agent_transaction_event_id` BIGINT NOT NULL,
    `sequence` BIGINT NOT NULL,
    `created_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    `updated_at` TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_agent_transaction_event_sequences_parent (`agent_transaction_event_id`),
    INDEX idx_agent_transaction_event_sequences_value (`sequence`),
    FOREIGN KEY (`agent_transaction_event_id`) REFERENCES agent_transaction_event (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
