# Silvana RPC - Proto-driven Database Schema Management
# =======================================================
# This Makefile implements the workflow where proto files are the single source of truth

# Configuration
PROTO_FILES := proto/events.proto
SQL_DIR := tidb/sql
MIGR_DIR := tidb/migration/sql
ENTITY_DIR := src/entity

# Database configuration check
check-database-url:
	@if [ -z "$(DATABASE_URL)" ]; then \
		echo "❌ ERROR: DATABASE_URL environment variable is not set"; \
		echo ""; \
		echo "Please set DATABASE_URL with the format:"; \
		echo "  export DATABASE_URL=\"mysql://user:pass@tcp(host:port)/database\""; \
		echo ""; \
		echo "Examples:"; \
		echo "  export DATABASE_URL=\"mysql://root:@tcp(localhost:4000)/silvana_rpc\""; \
		echo "  export DATABASE_URL=\"mysql://user:pass@tcp(myhost.com:3306)/mydb\""; \
		echo ""; \
		exit 1; \
	fi

# mysqldef supports DATABASE_URL directly, no parsing needed

.PHONY: help install-tools regen proto2sql entities clean-dev setup check-tools check-database-url validate-schema check-schema show-tables show-schema apply-ddl proto2entities dev-reset

# Default target
help: ## Show this help message
	@echo "Silvana RPC Database Schema Management"
	@echo "====================================="
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Environment Variables:"
	@echo "  DATABASE_URL    Database connection URL (REQUIRED)"
	@echo "                  Format: mysql://user:pass@tcp(host:port)/database"
	@echo ""
	@echo "Examples:"
	@echo "  export DATABASE_URL=\"mysql://root:@tcp(localhost:4000)/silvana_rpc\""
	@echo "  export DATABASE_URL=\"mysql://user:pass@tcp(myhost.com:3306)/mydb\""
	@echo ""
	@if [ -n "$(DATABASE_URL)" ]; then \
		echo "Current DATABASE_URL: $(DATABASE_URL)"; \
	else \
		echo "❌ DATABASE_URL is not set"; \
	fi

install-tools: ## Install required tools (mysqldef only - proto-to-ddl builds automatically)
	@echo "🔧 Installing required tools..."
	@echo "Installing mysqldef..."
	go install github.com/sqldef/sqldef/cmd/mysqldef@latest
	@echo "✅ All tools installed"
	@echo "ℹ️  proto-to-ddl will be built automatically via 'cargo run --release'"

check-tools: ## Check if required tools are installed
	@echo "🔍 Checking required tools..."
	@test -d tidb/proto-to-ddl || (echo "❌ proto-to-ddl directory not found" && exit 1)
	@command -v mysqldef >/dev/null 2>&1 || (echo "❌ mysqldef not found. Run 'make install-tools'" && exit 1)
	@echo "✅ All required tools are available"

setup: ## Create necessary directories
	@echo "📁 Setting up directories..."
	@mkdir -p $(SQL_DIR)
	@mkdir -p $(MIGR_DIR)
	@mkdir -p $(ENTITY_DIR)
	@echo "✅ Directories created"

regen: check-database-url check-tools setup proto2entities apply-ddl ## Complete regeneration: proto → DDL+entities → DB
	@echo "🎉 Regeneration complete!"
	@echo ""
	@echo "📝 Summary:"
	@echo "  - DDL generated from proto files"
	@echo "  - Database schema updated"
	@echo "  - Sea-ORM entities regenerated"
	@echo ""
	@echo "🚀 Your application is ready to use the updated schema!"

proto2sql: check-database-url ## Generate DDL from proto files and apply to database
	@echo "🔄 Generating DDL from proto files..."
	@mkdir -p $(SQL_DIR)
	cargo run --manifest-path tidb/proto-to-ddl/Cargo.toml --release -- generate \
		--proto-file $(PROTO_FILES) \
		--output $(SQL_DIR)/events.sql
	@echo "✅ DDL generated in $(SQL_DIR)/events.sql"
	@echo ""
	@echo "📊 Applying schema changes to database..."
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	mysqldef --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--file $(SQL_DIR)/events.sql \
		--dry-run > $(MIGR_DIR)/$$(date +%s)_proto_diff.sql
	@echo "🔍 Migration diff saved to $(MIGR_DIR)/"
	@echo "📊 Applying changes to database..."
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	mysqldef --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--file $(SQL_DIR)/events.sql
	@echo "✅ Database schema updated"

proto2entities: ## Generate both DDL and entities from proto files (combined)
	@echo "🔄 Generating DDL and entities from proto files..."
	@mkdir -p $(SQL_DIR)
	@rm -rf $(ENTITY_DIR)/*
	cargo run --manifest-path tidb/proto-to-ddl/Cargo.toml --release -- generate \
		--proto-file $(PROTO_FILES) \
		--output $(SQL_DIR)/events.sql \
		--entities \
		--entity-dir $(ENTITY_DIR)
	@echo "✅ Generated DDL in $(SQL_DIR)/events.sql"
	@echo "✅ Generated entities in $(ENTITY_DIR)/"

apply-ddl: check-database-url ## Apply generated DDL to database
	@echo "📊 Applying schema changes to database..."
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	mysqldef --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--file $(SQL_DIR)/events.sql \
		--dry-run > $(MIGR_DIR)/$$(date +%s)_proto_diff.sql
	@echo "🔍 Migration diff saved to $(MIGR_DIR)/"
	@echo "📊 Applying changes to database..."
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	mysqldef --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--file $(SQL_DIR)/events.sql
	@echo "✅ Database schema updated"

entities: ## Generate Sea-ORM entities from proto file
	@echo "🔄 Generating Sea-ORM entities from proto file..."
	@rm -rf $(ENTITY_DIR)/*
	cargo run --manifest-path tidb/proto-to-ddl/Cargo.toml --release -- generate \
		--proto-file $(PROTO_FILES) \
		--output /dev/null \
		--entities \
		--entity-dir $(ENTITY_DIR)
	@echo "✅ Entities generated in $(ENTITY_DIR)/"

clean-dev: ## Drop all tables for fast development iteration
	@echo "⚠️  Development cleanup: Dropping all tables..."
	@echo "🗑️  This will completely wipe the database!"
	@read -p "Are you sure? (y/N): " confirm && [ "$$confirm" = "y" ] || exit 1
	@echo "🔄 Dropping all tables..."
	cargo run --manifest-path tidb/drop_all_tables/Cargo.toml
	@echo "🗑️  Removing generated directories..."
	@rm -rf tidb/migration/
	@rm -rf tidb/sql/
	@rm -rf src/entity/
	@echo "✅ Generated directories removed"
	@echo "✅ All tables dropped"
	@echo ""
	@echo "💡 Run 'make regen' to recreate schema from proto files"

# Development targets
dev-reset: clean-dev regen ## Full development reset: drop all tables + regenerate from proto

# Utility targets
show-tables: check-database-url ## Show all tables in the database
	@echo "📋 Tables in database:"
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	mysql --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--execute="SHOW TABLES;" 2>/dev/null || echo "❌ Could not connect to database"

show-schema: check-database-url ## Show schema for all tables  
	@echo "📊 Database schema:"
	@export DB_USER=$$(echo "$(DATABASE_URL)" | sed 's|mysql://||' | sed 's|:.*||'); \
	export DB_PASS=$$(echo "$(DATABASE_URL)" | sed 's|mysql://[^:]*:||' | sed 's|@.*||'); \
	export DB_HOST=$$(echo "$(DATABASE_URL)" | sed 's|.*@||' | sed 's|:.*||'); \
	export DB_PORT=$$(echo "$(DATABASE_URL)" | sed 's|.*:||' | sed 's|/.*||'); \
	export DB_NAME=$$(echo "$(DATABASE_URL)" | sed 's|.*/||'); \
	for table in $$(mysql --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
		--execute="SHOW TABLES;" --batch --skip-column-names 2>/dev/null); do \
		echo ""; \
		echo "🔍 Table: $$table"; \
		echo "--------------------------------------------------"; \
		mysql --user=$$DB_USER --password=$$DB_PASS --host=$$DB_HOST --port=$$DB_PORT $$DB_NAME \
			--execute="DESCRIBE $$table;" 2>/dev/null || echo "❌ Could not describe table $$table"; \
	done || echo "❌ Could not retrieve schema"

validate-schema: check-database-url check-tools ## Validate that database schema matches proto definitions
	@echo "🔍 Validating schema consistency..."
	cargo run --manifest-path tidb/proto-to-ddl/Cargo.toml --release -- validate \
		--proto-file $(PROTO_FILES) \
		--database-url $(DATABASE_URL)

check-schema: check-database-url check-tools ## Quick schema validation check
	@./tidb/check_schema.sh 