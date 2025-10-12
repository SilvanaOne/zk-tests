# Makefile for hash project

include .env
export

DAR := .daml/dist/hash-v1-0.0.20.dar

.PHONY: help build clean upload parties updates get-update inspect

help:
	@echo "Hash - Helper Commands"
	@echo "======================"
	@echo ""
	@echo "Environment (from .env):"
	@echo "  APP_USER_API_URL:     $${APP_USER_API_URL:-<unset>}"
	@echo "  PARTY_APP_USER:       $${PARTY_APP_USER:-<unset>}"
	@echo ""
	@echo "Core Targets:"
	@echo "  make help          - Show this help"
	@echo "  make build         - Build the hash DAR"
	@echo "  make clean         - Remove build artifacts"
	@echo "  make inspect       - Show DAR package ID"
	@echo ""
	@echo "Package Management:"
	@echo "  make upload [DAR_FILE=<path>]  - Upload DAR to app-user node"
	@echo ""
	@echo "Party Management:"
	@echo "  make parties       - List parties visible to app-user"
	@echo ""
	@echo "Ledger Updates:"
	@echo "  make updates [OFFSET=n]        - Show last 100 updates from app-user"
	@echo "  make get-update UPDATE_ID=<id> - Get details of a specific update"

build:
	@daml build

clean:
	@rm -rf .daml/dist

# Upload DAR to app-user node
upload:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	DAR_TO_UPLOAD=$${DAR_FILE:-$(DAR)}; \
	if [ ! -f "$$DAR_TO_UPLOAD" ]; then \
		echo "Error: DAR file not found: $$DAR_TO_UPLOAD"; \
		echo "Build first (make build) or pass DAR_FILE=<path>"; \
		exit 1; \
	fi; \
	echo "Uploading DAR file $$DAR_TO_UPLOAD to app-user..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_USER_API_URL)v2/packages \
		--data-binary "@$$DAR_TO_UPLOAD"); \
	echo "Response (raw):"; echo "$$RESP"

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
	@daml damlc inspect-dar $(DAR) | grep "hash-v1-0.0.20" | tail -1 | awk '{print "Package ID: " $$NF}' | tr -d '"'
