# Makefile for advanced-payment project

include cli/.env
export

DAR := daml/.daml/dist/advanced-payment-v1-0.0.1.dar
# Extract package name from DAR filename
PACKAGE_NAME := $(basename $(notdir $(DAR)))
# Extract package name without version
PACKAGE_BASE := $(shell echo $(PACKAGE_NAME) | sed 's/-[0-9].*//')

.PHONY: help build clean upload upload-grpc inspect packages-user packages-provider package-status-user package-status-provider vetting-status list cli

help:
	@echo "Advanced Payment - Helper Commands"
	@echo "==================================="
	@echo ""
	@echo "Environment (from cli/.env):"
	@echo "  APP_USER_API_URL:           $${APP_USER_API_URL:-<unset>}"
	@echo "  PARTY_APP_USER:             $${PARTY_APP_USER:-<unset>}"
	@echo "  PARTY_APP_PROVIDER:         $${PARTY_APP_PROVIDER:-<unset>}"
	@echo "  ADVANCED_PAYMENT_PACKAGE_ID: $${ADVANCED_PAYMENT_PACKAGE_ID:-<unset>}"
	@echo ""
	@echo "Core Targets:"
	@echo "  make help          - Show this help"
	@echo "  make build         - Build the DAML DAR"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make inspect       - Show DAR package ID"
	@echo ""
	@echo "Deployment:"
	@echo "  make upload        - Upload DAR via HTTP (may fail if DAR > 1MB)"
	@echo "  make upload-grpc   - Upload DAR via gRPC (bypasses nginx size limits)"
	@echo ""
	@echo "Package Management:"
	@echo "  make packages-user             - List packages on app-user node"
	@echo "  make packages-provider         - List packages on app-provider node"
	@echo "  make package-status-user       - Get package status on app-user"
	@echo "  make package-status-provider   - Get package status on app-provider"
	@echo "  make vetting-status            - Check vetting status for both parties"
	@echo ""
	@echo "CLI:"
	@echo "  make cli           - Build the Rust CLI"
	@echo "  make list          - List amulets for both parties"

build:
	@cd daml && daml build

clean:
	@rm -rf daml/.daml/dist

cli:
	@cd cli && cargo build

list:
	@cd cli && cargo run -- list

# Upload DAR to both app-user and app-provider nodes
upload:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in cli/.env"; \
		exit 1; \
	fi; \
	if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in cli/.env"; \
		exit 1; \
	fi; \
	if [ ! -f "$(DAR)" ]; then \
		echo "Error: DAR file not found: $(DAR)"; \
		echo "Build first with: make build"; \
		exit 1; \
	fi; \
	echo "Uploading DAR file $(DAR) to app-provider..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_PROVIDER_API_URL)v2/packages \
		--data-binary "@$(DAR)"); \
	echo "App-provider response: $$RESP"; \
	echo ""; \
	echo "Uploading DAR file $(DAR) to app-user..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_USER_API_URL)v2/packages \
		--data-binary "@$(DAR)"); \
	echo "App-user response: $$RESP"; \
	echo ""; \
	echo "Now run 'make inspect' to get the package ID and update ADVANCED_PAYMENT_PACKAGE_ID in cli/.env"

# Upload DAR via gRPC (bypasses nginx size limits)
upload-grpc:
	@if [ -z "$(GRPC_LEDGER_API_HOST)" ] || [ -z "$(GRPC_LEDGER_API_PORT)" ]; then \
		echo "Error: GRPC_LEDGER_API_HOST or GRPC_LEDGER_API_PORT not set in cli/.env"; \
		exit 1; \
	fi; \
	if [ ! -f "$(DAR)" ]; then \
		echo "Error: DAR file not found: $(DAR)"; \
		echo "Build first with: make build"; \
		exit 1; \
	fi; \
	echo "Uploading DAR file $(DAR) via gRPC to $(GRPC_LEDGER_API_HOST):$(GRPC_LEDGER_API_PORT)..."; \
	daml ledger upload-dar \
		--host $(GRPC_LEDGER_API_HOST) \
		--port $(GRPC_LEDGER_API_PORT) \
		--access-token-file $(ACCESS_TOKEN_FILE) \
		$(DAR); \
	echo ""; \
	echo "Now run 'make inspect' to get the package ID and update ADVANCED_PAYMENT_PACKAGE_ID in cli/.env"

# Inspect DAR package ID
inspect:
	@echo "Inspecting DAR: $(DAR)"
	@daml damlc inspect-dar $(DAR) | grep "$(PACKAGE_NAME)" | tail -1 | awk '{print "Package ID: " $$NF}' | tr -d '"'
	@echo ""
	@echo "Update ADVANCED_PAYMENT_PACKAGE_ID in cli/.env with this value"

# List packages on app-user
packages-user:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in cli/.env"; \
		exit 1; \
	fi
	@echo "Fetching packages from app-user..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/packages | jq '.packageIds[]' 2>/dev/null || \
		echo "Error: Unable to fetch packages"

# List packages on app-provider
packages-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in cli/.env"; \
		exit 1; \
	fi
	@echo "Fetching packages from app-provider..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/packages | jq '.packageIds[]' 2>/dev/null || \
		echo "Error: Unable to fetch packages"

# Get package status on app-user
package-status-user:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(ADVANCED_PAYMENT_PACKAGE_ID)" ]; then \
		echo "Error: APP_USER_API_URL, APP_USER_JWT, or ADVANCED_PAYMENT_PACKAGE_ID not set in cli/.env"; \
		exit 1; \
	fi
	@echo "Fetching package status for $(ADVANCED_PAYMENT_PACKAGE_ID) from app-user..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/packages/$(ADVANCED_PAYMENT_PACKAGE_ID)/status" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch package status"

# Get package status on app-provider
package-status-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(ADVANCED_PAYMENT_PACKAGE_ID)" ]; then \
		echo "Error: APP_PROVIDER_API_URL, APP_PROVIDER_JWT, or ADVANCED_PAYMENT_PACKAGE_ID not set in cli/.env"; \
		exit 1; \
	fi
	@echo "Fetching package status for $(ADVANCED_PAYMENT_PACKAGE_ID) from app-provider..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/packages/$(ADVANCED_PAYMENT_PACKAGE_ID)/status" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch package status"

# Check vetting status from app-user perspective
vetting-status:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in cli/.env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for $(PACKAGE_BASE) with both parties..."
	@echo "Parties: $(PARTY_APP_USER), $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/interactive-submission/preferred-package-version?package-name=$(PACKAGE_BASE)&parties=$(PARTY_APP_USER)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"
