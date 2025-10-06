# Makefile for Canton SimpleSum Contract API Operations
include .env
export

# Default target - show help
.PHONY: help
help:
	@echo "Canton SimpleSum Contract - JSON Ledger API v2 Commands"
	@echo "========================================================"
	@echo ""
	@echo "Basic Commands:"
	@echo "  make parties-user    - List parties visible to app-user"
	@echo "  make parties-provider - List parties visible to app-provider"
	@echo ""
	@echo "DAR Management:"
	@echo "  make upload DAR_FILE=<path> - Upload DAR to app-user"
	@echo "  make upload-provider DAR_FILE=<path> - Upload DAR to app-provider"
	@echo ""
	@echo "Contract Operations:"
	@echo "  make create          - Create SimpleSumContract with app-user as prover"
	@echo "  make add CONTRACT_ID=<id> VALUE=<int> - Add value to the sum"
	@echo "  make observer CONTRACT_ID=<id> - Add app-provider as observer"
	@echo ""
	@echo "Ledger Queries:"
	@echo "  make updates         - Get ledger updates (last 100 by default)"
	@echo "  make updates OFFSET=<n> - Get updates from specific offset"
	@echo ""
	@echo "Environment:"
	@echo "  APP_USER_API_URL: $(APP_USER_API_URL)"
	@echo "  APP_PROVIDER_API_URL: $(APP_PROVIDER_API_URL)"
	@echo "  PACKAGE_ID: $(PACKAGE_ID)"

# List parties visible to app-user
.PHONY: parties-user
parties-user:
	@echo "Fetching parties visible to app-user from $(APP_USER_API_URL)..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/parties | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch parties"

# List parties visible to app-provider
.PHONY: parties-provider
parties-provider:
	@echo "Fetching parties visible to app-provider from $(APP_PROVIDER_API_URL)..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/parties | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch parties"

# Upload DAR to app-user
.PHONY: upload
upload:
	@if [ -z "$(DAR_FILE)" ]; then \
		echo "Usage: make upload DAR_FILE=<path-to-dar>"; \
		echo "  Example: make upload DAR_FILE=.daml/dist/testapp-0.0.1.dar"; \
		exit 1; \
	fi
	@echo "Uploading DAR file $(DAR_FILE) to app-user..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_USER_API_URL)v2/packages \
		--data-binary "@$(DAR_FILE)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to upload DAR file to app-user"

# Upload DAR to app-provider
.PHONY: upload-provider
upload-provider:
	@if [ -z "$(DAR_FILE)" ]; then \
		echo "Usage: make upload-provider DAR_FILE=<path-to-dar>"; \
		echo "  Example: make upload-provider DAR_FILE=.daml/dist/testapp-0.0.1.dar"; \
		exit 1; \
	fi
	@echo "Uploading DAR file $(DAR_FILE) to app-provider..."
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/octet-stream" \
		-X POST $(APP_PROVIDER_API_URL)v2/packages \
		--data-binary "@$(DAR_FILE)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to upload DAR file to app-provider"

# Create SimpleSumContract with app-user as prover
.PHONY: create
create:
	@echo "Creating SimpleSumContract with app-user as prover..."
	@if [ -z "$(APP_PACKAGE_ID)" ]; then \
		echo "Error: PACKAGE_ID not set in .env file"; \
		echo "Upload the DAR first and set PACKAGE_ID in .env"; \
		exit 1; \
	fi
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_USER_API_URL)v2/commands/submit-and-wait-for-transaction \
		-d '{ \
			"commands": { \
				"commands": [ \
					{ \
						"CreateCommand": { \
							"templateId": "$(APP_PACKAGE_ID):Main:SimpleSumContract", \
							"createArguments": { \
								"prover": "$(PARTY_APP_USER)", \
								"sum": 0, \
								"observers": [] \
							} \
						} \
					} \
				], \
				"commandId": "create-'$$(date +%s)'", \
				"actAs": ["$(PARTY_APP_USER)"] \
			} \
		}' | jq '.' 2>/dev/null || echo "Error: Unable to create contract"

# Exercise AddValue choice to add a value to the sum
.PHONY: add
add:
	@if [ -z "$(CONTRACT_ID)" ] || [ -z "$(VALUE)" ]; then \
		echo "Usage: make add CONTRACT_ID=<id> VALUE=<int>"; \
		echo "  Example: make add CONTRACT_ID=001234... VALUE=50"; \
		exit 1; \
	fi
	@if [ -z "$(APP_PACKAGE_ID)" ]; then \
		echo "Error: PACKAGE_ID not set in .env file"; \
		exit 1; \
	fi
	@echo "Adding value $(VALUE) to contract $(CONTRACT_ID)..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_USER_API_URL)v2/commands/submit-and-wait-for-transaction \
		-d '{ \
			"commands": { \
				"commands": [ \
					{ \
						"ExerciseCommand": { \
							"templateId": "$(APP_PACKAGE_ID):Main:SimpleSumContract", \
							"contractId": "$(CONTRACT_ID)", \
							"choice": "AddValue", \
							"choiceArgument": { \
								"value": $(VALUE) \
							} \
						} \
					} \
				], \
				"commandId": "add-value-'$$(date +%s)'", \
				"actAs": ["$(PARTY_APP_USER)"] \
			} \
		}' | jq '.' 2>/dev/null || echo "Error: Unable to add value"

# Exercise AddObserver choice to add app-provider as observer
.PHONY: observer
observer:
	@if [ -z "$(CONTRACT_ID)" ]; then \
		echo "Usage: make observer CONTRACT_ID=<id>"; \
		echo "  Example: make observer CONTRACT_ID=001234..."; \
		echo "  This will add app-provider as an observer to the contract"; \
		exit 1; \
	fi
	@if [ -z "$(APP_PACKAGE_ID)" ]; then \
		echo "Error: PACKAGE_ID not set in .env file"; \
		exit 1; \
	fi
	@echo "Adding app-provider as observer to contract $(CONTRACT_ID)..."
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_USER_API_URL)v2/commands/submit-and-wait-for-transaction \
		-d '{ \
			"commands": { \
				"commands": [ \
					{ \
						"ExerciseCommand": { \
							"templateId": "$(APP_PACKAGE_ID):Main:SimpleSumContract", \
							"contractId": "$(CONTRACT_ID)", \
							"choice": "AddObserver", \
							"choiceArgument": { \
								"newObserver": "$(PARTY_APP_PROVIDER)" \
							} \
						} \
					} \
				], \
				"commandId": "add-observer-'$$(date +%s)'", \
				"actAs": ["$(PARTY_APP_USER)"] \
			} \
		}' | jq '.' 2>/dev/null || echo "Error: Unable to add observer"

# Get ledger updates from a specific offset (last 100 updates by default)
.PHONY: updates
updates:
	@if [ -z "$(OFFSET)" ]; then \
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
	echo "Fetching updates after offset $$OFFSET..."; \
	curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/updates?limit=100&timeout=5" \
		-d '{ \
			"beginExclusive": '"$$OFFSET"', \
			"endInclusive": null, \
			"filter": { \
				"filtersByParty": { \
					"$(PARTY_APP_USER)": {} \
				} \
			}, \
			"verbose": true \
		}' | jq '.' 2>/dev/null || echo "Error: Unable to fetch updates"