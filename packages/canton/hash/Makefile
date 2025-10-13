# Makefile for hash project

include .env
export

DAR := .daml/dist/hash-v8-0.0.1.dar

.PHONY: help build clean upload parties parties-provider updates get-update inspect packages-user packages-provider package-status-user package-status-provider vetting-status-user vetting-status-provider traffic

help:
	@echo "Hash - Helper Commands"
	@echo "======================"
	@echo ""
	@echo "Environment (from .env):"
	@echo "  APP_USER_API_URL:     $${APP_USER_API_URL:-<unset>}"
	@echo "  PARTY_APP_USER:       $${PARTY_APP_USER:-<unset>}"
	@echo "  HASH_PACKAGE_ID:      $${HASH_PACKAGE_ID:-<unset>}"
	@echo ""
	@echo "Core Targets:"
	@echo "  make help          - Show this help"
	@echo "  make build         - Build the hash DAR"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make inspect       - Show DAR package ID"
	@echo ""
	@echo "Package Management:"
	@echo "  make upload [DAR_FILE=<path>]  - Upload DAR to both user and provider nodes"
	@echo "  make packages-user             - List packages on app-user node"
	@echo "  make packages-provider         - List packages on app-provider node"
	@echo "  make package-status-user       - Get hash-v6 package status on app-user"
	@echo "  make package-status-provider   - Get hash-v6 package status on app-provider"
	@echo "  make vetting-status-user       - Check vetting status for both parties (from app-user)"
	@echo "  make vetting-status-provider   - Check vetting status for both parties (from app-provider)"
	@echo ""
	@echo "Party Management:"
	@echo "  make parties          - List parties visible to app-user"
	@echo "  make parties-provider - List parties visible to app-provider"
	@echo ""
	@echo "Ledger Updates:"
	@echo "  make updates [OFFSET=n]        - Show last 100 updates from app-user"
	@echo "  make get-update UPDATE_ID=<id> - Get details of a specific update"
	@echo ""
	@echo "Traffic Monitoring:"
	@echo "  make traffic                   - Show current traffic status for user participant"

build:
	@daml build

clean:
	@rm -rf .daml/dist

# Upload DAR to both app-user and app-provider nodes
upload:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi; \
	DAR_TO_UPLOAD=$${DAR_FILE:-$(DAR)}; \
	if [ ! -f "$$DAR_TO_UPLOAD" ]; then \
		echo "Error: DAR file not found: $$DAR_TO_UPLOAD"; \
		echo "Build first (make build) or pass DAR_FILE=<path>"; \
		exit 1; \
	fi; \
	echo "Uploading DAR file $$DAR_TO_UPLOAD to app-provider..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_PROVIDER_API_URL)v2/packages \
		--data-binary "@$$DAR_TO_UPLOAD"); \
	echo "App-provider response: $$RESP"; \
	echo ""; \
	echo "Uploading DAR file $$DAR_TO_UPLOAD to app-user..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_USER_API_URL)v2/packages \
		--data-binary "@$$DAR_TO_UPLOAD"); \
	echo "App-user response: $$RESP"; 
	


# List parties visible to app-user
parties:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching parties visible to app-user from $(APP_USER_API_URL)..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/parties | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch parties"

# List parties visible to app-provider
parties-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching parties visible to app-provider from $(APP_PROVIDER_API_URL)..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/parties | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch parties"

# Get last updates (default last 100) for app-user
updates:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	if [ -z "$(OFFSET)" ]; then \
		echo "Getting ledger end to determine offset..."; \
		LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
			$(APP_USER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
		if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
			echo "Error: Unable to get ledger end offset"; \
			exit 1; \
		fi; \
		echo "Current ledger end offset: $$LEDGER_END"; \
		if [ "$$LEDGER_END" -gt 100 ]; then \
			OFFSET=$$(($$LEDGER_END - 100)); \
			echo "Fetching last 100 updates (from offset $$OFFSET to $$LEDGER_END)"; \
		else \
			OFFSET=0; \
			echo "Fetching all updates from beginning (ledger has less than 100 updates)"; \
		fi; \
	else \
		OFFSET=$(OFFSET); \
		echo "Using specified offset: $$OFFSET"; \
	fi; \
	FILTER_BY_PARTY=$${PARTY_APP_USER:+"\"$$PARTY_APP_USER\": {}"}; \
	if [ -z "$$FILTER_BY_PARTY" ]; then \
		FILTER_JSON='{}'; \
	else \
		FILTER_JSON='{"filtersByParty": {'"$$FILTER_BY_PARTY"'}}'; \
	fi; \
	echo "Fetching updates after offset $$OFFSET..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/updates?limit=100&timeout=5" \
		-d "$$(jq -nc --arg offset "$$OFFSET" --argjson filter "$$FILTER_JSON" \
			'{beginExclusive: $$offset, filter: $$filter, verbose: true}')"); \
	echo "$$RESP" | jq '.' 2>/dev/null || echo "$$RESP"

# Get update by ID (POST method)
get-update:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	if [ -z "$(UPDATE_ID)" ]; then \
		echo "Error: UPDATE_ID not specified. Usage: make get-update UPDATE_ID=<id>"; \
		exit 1; \
	fi; \
	echo "Fetching update $(UPDATE_ID)..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_USER_API_URL)v2/updates/update-by-id \
		-d '{"actAs": ["$(PARTY_APP_USER)"], "updateId": "$(UPDATE_ID)", "updateFormat": {"includeTransactions": {"eventFormat": {"filtersByParty": {"$(PARTY_APP_USER)": {}}, "verbose": true}, "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"}}}'); \
	echo "$$RESP" | jq '.' 2>/dev/null || echo "$$RESP"

# Inspect DAR package ID
inspect:
	@echo "Inspecting DAR: $(DAR)"
	@daml damlc inspect-dar $(DAR) | grep "hash-v8-0.0.1" | tail -1 | awk '{print "Package ID: " $$NF}' | tr -d '"'

# List packages on app-user
packages-user:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching packages from app-user..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/packages | jq '.packageIds[]' 2>/dev/null || \
		echo "Error: Unable to fetch packages"

# List packages on app-provider
packages-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching packages from app-provider..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/packages | jq '.packageIds[]' 2>/dev/null || \
		echo "Error: Unable to fetch packages"

# Get package status on app-user
package-status-user:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(HASH_PACKAGE_ID)" ]; then \
		echo "Error: APP_USER_API_URL, APP_USER_JWT, or HASH_PACKAGE_ID not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching package status for $(HASH_PACKAGE_ID) from app-user..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/packages/$(HASH_PACKAGE_ID)/status" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch package status"

# Get package status on app-provider
package-status-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(HASH_PACKAGE_ID)" ]; then \
		echo "Error: APP_PROVIDER_API_URL, APP_PROVIDER_JWT, or HASH_PACKAGE_ID not set in .env"; \
		exit 1; \
	fi
	@echo "Fetching package status for $(HASH_PACKAGE_ID) from app-provider..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/packages/$(HASH_PACKAGE_ID)/status" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch package status"

# Check vetting status from app-user perspective
vetting-status-user:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for hash-v6 with both parties from app-user perspective..."
	@echo "Parties: $(PARTY_APP_USER), $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/interactive-submission/preferred-package-version?package-name=hash-v6&parties=$(PARTY_APP_USER)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting status from app-provider perspective
vetting-status-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for hash-v6 with both parties from app-provider perspective..."
	@echo "Parties: $(PARTY_APP_USER), $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/interactive-submission/preferred-package-version?package-name=hash-v6&parties=$(PARTY_APP_USER)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting for app_user party only
vetting-status-user-only:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for hash-v8 with app_user party only..."
	@echo "Party: $(PARTY_APP_USER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/interactive-submission/preferred-package-version?package-name=hash-v8&parties=$(PARTY_APP_USER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting for app_provider party only
vetting-status-provider-only:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for hash-v8 with app_provider party only..."
	@echo "Party: $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/interactive-submission/preferred-package-version?package-name=hash-v8&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

traffic:
	@echo "Canton Synchronizer Traffic Status"
	@echo "==================================="
	@echo ""
	@if [ -z "$(SCAN_API_URL)" ] || [ -z "$(PARTY_PARTICIPANT_USER)" ]; then \
		echo "Error: SCAN_API_URL and PARTY_PARTICIPANT_USER must be set in .env"; \
		exit 1; \
	fi
	@ENCODED_DOMAIN=$$(echo "$(SYNCHRONIZER_ID)" | sed 's/:/%3A/g'); \
	MEMBER_ID="PAR::$(PARTY_PARTICIPANT_USER)"; \
	ENCODED_MEMBER=$$(echo "$$MEMBER_ID" | sed 's/:/%3A/g'); \
	echo "User Participant: $(PARTY_PARTICIPANT_USER)"; \
	echo "Domain: $(SYNCHRONIZER_ID)"; \
	echo "Member ID: $$MEMBER_ID"; \
	echo ""; \
	echo "Fetching traffic status..."; \
	echo ""; \
	RESPONSE=$$(curl -s "$(SCAN_API_URL)v0/domains/$$ENCODED_DOMAIN/members/$$ENCODED_MEMBER/traffic-status"); \
	echo "Raw Response:"; \
	echo "$$RESPONSE" | jq '.' 2>/dev/null || echo "$$RESPONSE"; \
	echo ""; \
	echo "Parsed:"; \
	echo "$$RESPONSE" | jq -r '. as $$root | if .error then "Error: " + .error else "  Total Consumed:  \($$root.traffic_status.actual.total_consumed) bytes\n  Total Limit:     \($$root.traffic_status.actual.total_limit) bytes\n  Total Purchased: \($$root.traffic_status.target.total_purchased) bytes\n  Available:       \($$root.traffic_status.actual.total_limit - $$root.traffic_status.actual.total_consumed) bytes" end' 2>/dev/null || echo "Error: Unable to parse traffic status"
