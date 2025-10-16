# Makefile for hash project

include .env
export

DAR := .daml/dist/hash-v34-0.0.1.dar
# Extract package name from DAR filename (e.g., hash-v15-0.0.1.dar -> hash-v15-0.0.1)
PACKAGE_NAME := $(basename $(notdir $(DAR)))
# Extract package name without version (e.g., hash-v15-0.0.1 -> hash-v15)
PACKAGE_BASE := $(shell echo $(PACKAGE_NAME) | sed 's/-[0-9].*//')

.PHONY: help build clean upload parties parties-provider create-party updates get-update get-update-provider inspect packages-user packages-provider package-status-user package-status-provider vetting-status-user vetting-status-provider traffic get-package decode-package find-amulets split-amulets merge-amulets extract-contract-blobs tokens find-rewards find-markers

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
	@echo "  make get-package               - Download and verify DALF package from Canton"
	@echo "  make decode-package [DALF=<file>] - Decode and inspect DALF file"
	@echo "  make packages-user             - List packages on app-user node"
	@echo "  make packages-provider         - List packages on app-provider node"
	@echo "  make package-status-user       - Get $(PACKAGE_BASE) package status on app-user"
	@echo "  make package-status-provider   - Get $(PACKAGE_BASE) package status on app-provider"
	@echo "  make vetting-status-user       - Check vetting status for both parties (from app-user)"
	@echo "  make vetting-status-provider   - Check vetting status for both parties (from app-provider)"
	@echo ""
	@echo "Party Management:"
	@echo "  make parties          - List parties visible to app-user"
	@echo "  make parties-provider - List parties visible to app-provider"
	@echo "  make create-party [PARTY_HINT=<name>] - Create a party on app-user"
	@echo ""
	@echo "Ledger Updates:"
	@echo "  make updates [OFFSET=n]                 - Show last 100 updates from app-user"
	@echo "  make get-update UPDATE_ID=<id>          - Get details of a specific update (user view)"
	@echo "  make get-update-provider UPDATE_ID=<id> - Get details of a specific update (provider view)"
	@echo ""
	@echo "Traffic Monitoring:"
	@echo "  make traffic                   - Show current traffic status for user participant"
	@echo ""
	@echo "Amulet Contracts:"
	@echo ""
	@echo "TestToken Management:"
	@echo "  make tokens                     - Show TestToken balances using mint.env data"
	@echo "  make find-amulets [PARTY=<party>] - Find Amulet contracts for party (defaults to PARTY_APP_USER)"
	@echo "  make split-amulets              - Split largest amulet into 5x 10 CC + remainder"
	@echo "  make merge-amulets              - Merge all amulets into a single amulet"
	@echo ""
	@echo "Rewards:"
	@echo "  make find-rewards               - Find FeaturedAppActivityMarkers and AppRewardCoupons for provider"
	@echo "  make find-markers               - Find FeaturedAppActivityMarkers for provider (detailed view)"

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

# Create a new party on app-user node
create-party:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	if [ -z "$(PARTY_HINT)" ]; then \
		PARTY_HINT=userParty; \
		echo "PARTY_HINT not provided. Using default: $$PARTY_HINT"; \
	fi; \
	echo "Allocating party with hint '$$PARTY_HINT' on app-user..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_USER_API_URL)v2/parties \
		-d '{"partyIdHint":"'"$$PARTY_HINT"'","identityProviderId":""}'); \
	echo "Response (raw):"; echo "$$RESP"; \
	NEW_PARTY=$$(echo "$$RESP" | jq -r '.partyDetails.party // .partyDetails.partyId // .party // .partyId // empty' 2>/dev/null); \
	if [ -n "$$NEW_PARTY" ]; then \
		echo "Created party: $$NEW_PARTY"; \
		echo "Export this in your environment if needed:"; \
		echo "  export PARTY_NEW=$$NEW_PARTY"; \
	else \
		echo "Warning: Could not parse created party id"; \
	fi

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

# Get update by ID from provider perspective (POST method)
get-update-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi; \
	if [ -z "$(UPDATE_ID)" ]; then \
		echo "Error: UPDATE_ID not specified. Usage: make get-update-provider UPDATE_ID=<id>"; \
		exit 1; \
	fi; \
	echo "Fetching update $(UPDATE_ID) from provider perspective..."; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST $(APP_PROVIDER_API_URL)v2/updates/update-by-id \
		-d '{"actAs": ["$(PARTY_APP_PROVIDER)"], "updateId": "$(UPDATE_ID)", "updateFormat": {"includeTransactions": {"eventFormat": {"filtersByParty": {"$(PARTY_APP_PROVIDER)": {}}, "verbose": true}, "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"}}}'); \
	echo "$$RESP" | jq '.' 2>/dev/null || echo "$$RESP"

# Inspect DAR package ID
inspect:
	@echo "Inspecting DAR: $(DAR)"
	@daml damlc inspect-dar $(DAR) | grep "$(PACKAGE_NAME)" | tail -1 | awk '{print "Package ID: " $$NF}' | tr -d '"'

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
	@echo "Checking vetting status for $(PACKAGE_BASE) with both parties from app-user perspective..."
	@echo "Parties: $(PARTY_APP_USER), $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/interactive-submission/preferred-package-version?package-name=$(PACKAGE_BASE)&parties=$(PARTY_APP_USER)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting status from app-provider perspective
vetting-status-provider:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for $(PACKAGE_BASE) with both parties from app-provider perspective..."
	@echo "Parties: $(PARTY_APP_USER), $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/interactive-submission/preferred-package-version?package-name=$(PACKAGE_BASE)&parties=$(PARTY_APP_USER)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting for app_user party only
vetting-status-user-only:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for $(PACKAGE_BASE) with app_user party only..."
	@echo "Party: $(PARTY_APP_USER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/interactive-submission/preferred-package-version?package-name=$(PACKAGE_BASE)&parties=$(PARTY_APP_USER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
		echo "Error: Unable to fetch vetting status"

# Check vetting for app_provider party only
vetting-status-provider-only:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ] || [ -z "$(PARTY_APP_PROVIDER)" ] || [ -z "$(SYNCHRONIZER_ID)" ]; then \
		echo "Error: Required environment variables not set in .env"; \
		exit 1; \
	fi
	@echo "Checking vetting status for $(PACKAGE_BASE) with app_provider party only..."
	@echo "Party: $(PARTY_APP_PROVIDER)"
	@echo "Synchronizer: $(SYNCHRONIZER_ID)"
	@echo ""
	@curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		"$(APP_PROVIDER_API_URL)v2/interactive-submission/preferred-package-version?package-name=$(PACKAGE_BASE)&parties=$(PARTY_APP_PROVIDER)&synchronizer-id=$(SYNCHRONIZER_ID)" | jq '.' 2>/dev/null || \
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

get-package:
	@if [ -z "$(HASH_PACKAGE_ID)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(APP_USER_API_URL)" ]; then \
		echo "Error: HASH_PACKAGE_ID, APP_USER_JWT, and APP_USER_API_URL must be set in .env"; \
		exit 1; \
	fi
	@echo "Downloading package $(HASH_PACKAGE_ID)..."
	@echo ""
	@curl -v -H "Authorization: Bearer $(APP_USER_JWT)" \
		"$(APP_USER_API_URL)v2/packages/$(HASH_PACKAGE_ID)" \
		-o "hash-package-$(HASH_PACKAGE_ID).dalf" 2>&1 | grep -E "Canton-Package-Hash|HTTP/|Content-"
	@echo ""
	@echo "Package downloaded to: hash-package-$(HASH_PACKAGE_ID).dalf"
	@ls -lh "hash-package-$(HASH_PACKAGE_ID).dalf"
	@echo ""
	@echo "Verifying SHA256 hash..."
	@ACTUAL_HASH=$$(sha256sum "hash-package-$(HASH_PACKAGE_ID).dalf" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "hash-package-$(HASH_PACKAGE_ID).dalf" | cut -d' ' -f1); \
	if [ "$$ACTUAL_HASH" = "$(HASH_PACKAGE_ID)" ]; then \
		echo "✅ Hash verified: $$ACTUAL_HASH"; \
	else \
		echo "❌ Hash mismatch!"; \
		echo "  Expected: $(HASH_PACKAGE_ID)"; \
		echo "  Actual:   $$ACTUAL_HASH"; \
	fi
	@echo ""
	@echo "Adding DAML-LF envelope for decoding..."
	@mv "hash-package-$(HASH_PACKAGE_ID).dalf" "hash-package-$(HASH_PACKAGE_ID).raw"
	@(printf '\x1a\xdf\xc6\x0a'; cat "hash-package-$(HASH_PACKAGE_ID).raw"; printf '\x22\x40%s' "$(HASH_PACKAGE_ID)") > "hash-package-$(HASH_PACKAGE_ID).dalf"
	@rm "hash-package-$(HASH_PACKAGE_ID).raw"
	@echo "✅ DAML-LF envelope added (4 + 66 bytes)"
	@echo ""
	@echo "Decoding DALF package..."
	@echo ""
	@daml damlc inspect "hash-package-$(HASH_PACKAGE_ID).dalf" 2>&1 || echo "Could not decode DALF"

find-amulets:
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	PARTY=$${PARTY:-$(PARTY_APP_USER)}; \
	if [ -z "$$PARTY" ]; then \
		echo "Error: PARTY not specified and PARTY_APP_USER not set"; \
		exit 1; \
	fi; \
	echo "Searching for Amulet contracts for party $$PARTY..."; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/state/active-contracts?limit=500" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$$PARTY" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	AMULETS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("Splice.Amulet:Amulet")))]' 2>/dev/null); \
	if [ "$$AMULETS" != "[]" ] && [ -n "$$AMULETS" ] && [ "$$AMULETS" != "null" ]; then \
		echo "Found Amulet contracts:"; \
		echo "$$AMULETS" | jq -r '.[] | "  Contract ID: \(.contractEntry.JsActiveContract.createdEvent.contractId)\n  Amount: \(.contractEntry.JsActiveContract.createdEvent.createArgument.amount.initialAmount) AMT\n  Created At Round: \(.contractEntry.JsActiveContract.createdEvent.createArgument.amount.createdAt.number // "N/A")"' 2>/dev/null; \
		echo ""; \
		CIDS=$$(echo "$$AMULETS" | jq -r '[.[] | .contractEntry.JsActiveContract.createdEvent.contractId] | join(",")' 2>/dev/null); \
		if [ -n "$$CIDS" ] && [ "$$CIDS" != "null" ]; then \
			echo "INPUT_AMULET_CIDS=$$CIDS"; \
		fi; \
	else \
		echo "No Amulet contracts found for party $$PARTY"; \
		echo ""; \
		echo "The party needs some Amulets first."; \
	fi

extract-contract-blobs:
	@if [ -z "$(SCAN_API_URL)" ]; then \
		echo "Error: SCAN_API_URL not set in .env"; \
		exit 1; \
	fi; \
	echo "Extracting contract blobs from Scan API..." >&2; \
	echo "" >&2; \
	echo "Fetching AmuletRules from /v0/dso..." >&2; \
	DSO_RESP=$$(curl -s "$(SCAN_API_URL)v0/dso"); \
	AMULET_BLOB=$$(echo "$$DSO_RESP" | jq -r '.amulet_rules.contract.created_event_blob // empty'); \
	AMULET_CID=$$(echo "$$DSO_RESP" | jq -r '.amulet_rules.contract.contract_id // empty'); \
	echo "  AmuletRules CID: $$AMULET_CID" >&2; \
	echo "Fetching current OpenMiningRound from /v0/open-and-issuing-mining-rounds..." >&2; \
	ROUNDS_RESP=$$(curl -s -H "Content-Type: application/json" \
		-X POST "$(SCAN_API_URL)v0/open-and-issuing-mining-rounds" \
		-d '{"cached_open_mining_round_contract_ids": [], "cached_issuing_round_contract_ids": []}' 2>/dev/null || echo "{}"); \
	if echo "$$ROUNDS_RESP" | jq -e '.open_mining_rounds' >/dev/null 2>&1; then \
		MINING_CID=$$(echo "$$ROUNDS_RESP" | jq -r '.open_mining_rounds | to_entries | sort_by(.value.payload.opensAt) | first | .value.contract.contract_id // empty' 2>/dev/null); \
		MINING_BLOB=$$(echo "$$ROUNDS_RESP" | jq -r '.open_mining_rounds | to_entries | sort_by(.value.payload.opensAt) | first | .value.contract.created_event_blob // empty' 2>/dev/null); \
		if [ -n "$$MINING_CID" ]; then \
			echo "  OpenMiningRound CID: $$MINING_CID (current round)" >&2; \
		else \
			echo "  Warning: Could not find current OpenMiningRound" >&2; \
		fi; \
	else \
		echo "  Warning: Could not fetch open mining rounds" >&2; \
		MINING_CID=""; \
		MINING_BLOB=""; \
	fi; \
	echo "" >&2; \
	echo "Extracting synchronizer ID..." >&2; \
	SYNC_ID=$$(echo "$$AMULET_BLOB" | base64 -d | strings | grep -o "global-domain::[a-f0-9]*" | head -1); \
	if [ -z "$$SYNC_ID" ]; then \
		echo "  Warning: Could not extract synchronizer ID from blob" >&2; \
		SYNC_ID=""; \
	else \
		echo "  Extracted synchronizer ID: $$SYNC_ID" >&2; \
	fi; \
	echo "" >&2; \
	echo "Writing to contract-blobs.env..." >&2; \
	{ \
		echo "# Contract blobs and CIDs extracted from Scan API"; \
		echo "# Generated on $$(date)"; \
		echo ""; \
		echo "# Contract IDs"; \
		echo "AMULET_RULES_CID=$$AMULET_CID"; \
		echo "OPEN_MINING_ROUND_CID=$$MINING_CID"; \
		echo ""; \
		echo "# Contract blobs for disclosure"; \
		echo "AMULET_RULES_BLOB=$$AMULET_BLOB"; \
		echo "OPEN_MINING_ROUND_BLOB=$$MINING_BLOB"; \
		echo ""; \
		echo "# Synchronizer ID"; \
		echo "SYNCHRONIZER_ID=$$SYNC_ID"; \
	} > contract-blobs.env; \
	echo "✅ Contract blobs saved to contract-blobs.env" >&2

split-amulets: extract-contract-blobs
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(SPLICE_PACKAGE_ID)" ]; then \
		echo "Error: APP_USER_API_URL, APP_USER_JWT, PARTY_APP_USER, and SPLICE_PACKAGE_ID must be set in .env"; \
		exit 1; \
	fi; \
	if [ ! -f contract-blobs.env ]; then \
		echo "Error: contract-blobs.env not found"; \
		exit 1; \
	fi; \
	source contract-blobs.env; \
	echo "Using contracts from contract-blobs.env:"; \
	echo "  AmuletRules: $$AMULET_RULES_CID"; \
	echo "  OpenMiningRound: $$OPEN_MINING_ROUND_CID"; \
	echo ""; \
	echo "Finding amulets for $(PARTY_APP_USER)..."; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/state/active-contracts?limit=500" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$(PARTY_APP_USER)" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	AMULETS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("Splice.Amulet:Amulet")) and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("LockedAmulet") | not))]' 2>/dev/null); \
	if [ "$$AMULETS" = "[]" ] || [ -z "$$AMULETS" ] || [ "$$AMULETS" = "null" ]; then \
		echo "No Amulet contracts found for party $(PARTY_APP_USER)"; \
		exit 1; \
	fi; \
	LARGEST=$$(echo "$$AMULETS" | jq 'sort_by(.contractEntry.JsActiveContract.createdEvent.createArgument.amount.initialAmount | tonumber) | reverse | .[0]'); \
	AMULET_CID=$$(echo "$$LARGEST" | jq -r '.contractEntry.JsActiveContract.createdEvent.contractId'); \
	AMOUNT=$$(echo "$$LARGEST" | jq -r '.contractEntry.JsActiveContract.createdEvent.createArgument.amount.initialAmount'); \
	echo "Found largest amulet: $$AMULET_CID"; \
	echo "Amount: $$AMOUNT CC"; \
	REMAINDER=$$(echo "$$AMOUNT - 50" | bc); \
	if [ $$(echo "$$REMAINDER < 0" | bc) -eq 1 ]; then \
		echo "Error: Amulet amount ($$AMOUNT CC) is less than 50 CC needed for split"; \
		exit 1; \
	fi; \
	echo "Splitting into 5x 10 CC + $$REMAINDER CC remainder..."; \
	echo ""; \
	echo "Building disclosed contracts array..."; \
	DISCLOSED_CONTRACTS="[]"; \
	if [ -n "$$AMULET_RULES_BLOB" ] && [ -n "$$SYNCHRONIZER_ID" ]; then \
		DISCLOSED_CONTRACTS=$$(echo "$$DISCLOSED_CONTRACTS" | jq --arg sync "$$SYNCHRONIZER_ID" '. + [{"templateId": "$(SPLICE_PACKAGE_ID):Splice.AmuletRules:AmuletRules", "contractId": "'"$$AMULET_RULES_CID"'", "createdEventBlob": "'"$$AMULET_RULES_BLOB"'", "synchronizerId": $$sync}]'); \
	fi; \
	if [ -n "$$OPEN_MINING_ROUND_BLOB" ] && [ -n "$$SYNCHRONIZER_ID" ]; then \
		DISCLOSED_CONTRACTS=$$(echo "$$DISCLOSED_CONTRACTS" | jq --arg sync "$$SYNCHRONIZER_ID" '. + [{"templateId": "$(SPLICE_PACKAGE_ID):Splice.Round:OpenMiningRound", "contractId": "'"$$OPEN_MINING_ROUND_CID"'", "createdEventBlob": "'"$$OPEN_MINING_ROUND_BLOB"'", "synchronizerId": $$sync}]'); \
	fi; \
	echo "  Disclosed contracts: $$(echo "$$DISCLOSED_CONTRACTS" | jq 'length') contracts"; \
	TRANSFER_RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/commands/submit-and-wait-for-transaction" \
		-d "$$(jq -nc \
			--arg amulet_cid "$$AMULET_CID" \
			--arg open_round "$$OPEN_MINING_ROUND_CID" \
			--arg party "$(PARTY_APP_USER)" \
			--arg remainder "$$REMAINDER" \
			--arg amulet_rules_cid "$$AMULET_RULES_CID" \
			--arg sync_id "$$SYNCHRONIZER_ID" \
			--argjson disclosed "$$DISCLOSED_CONTRACTS" \
			'{commands: {commands: [{ExerciseCommand: {templateId: "$(SPLICE_PACKAGE_ID):Splice.AmuletRules:AmuletRules", contractId: $$amulet_rules_cid, choice: "AmuletRules_Transfer", choiceArgument: {transfer: {sender: $$party, provider: $$party, inputs: [{tag: "InputAmulet", value: $$amulet_cid}], outputs: [{receiver: $$party, amount: "10.0", receiverFeeRatio: 0.0, lock: null}, {receiver: $$party, amount: "10.0", receiverFeeRatio: 0.0, lock: null}, {receiver: $$party, amount: "10.0", receiverFeeRatio: 0.0, lock: null}, {receiver: $$party, amount: "10.0", receiverFeeRatio: 0.0, lock: null}, {receiver: $$party, amount: "10.0", receiverFeeRatio: 0.0, lock: null}, {receiver: $$party, amount: $$remainder, receiverFeeRatio: 0.0, lock: null}], beneficiaries: null}, context: {openMiningRound: $$open_round, issuingMiningRounds: [], validatorRights: [], featuredAppRight: null}, expectedDso: "DSO::122075d227a0482dc186fa09a3ddc4e0b2046d1ce9fdf4ec7375bd698b362525632e"}}}], actAs: [$$party], readAs: [], disclosedContracts: $$disclosed, domainId: $$sync_id, workflowId: null, commandId: "split-\(now)", deduplicationPeriod: null, submissionId: null, minLedgerTimeAbs: null, minLedgerTimeRel: null, applicationId: "hash-split"}}')"); \
	if echo "$$TRANSFER_RESP" | jq -e '.error' >/dev/null 2>&1; then \
		echo "❌ Error splitting amulet:"; \
		echo "$$TRANSFER_RESP" | jq '.' 2>/dev/null || echo "$$TRANSFER_RESP"; \
		exit 1; \
	fi; \
	if echo "$$TRANSFER_RESP" | jq -e '.transaction' >/dev/null 2>&1; then \
		echo "✅ Amulet split successful!"; \
		echo ""; \
		NEW_AMULETS=$$(echo "$$TRANSFER_RESP" | jq -r '.transaction.events[]? | select(.CreatedEvent?) | .CreatedEvent | select(.templateId | contains("Splice.Amulet:Amulet")) | "  \(.contractId): \(.createArgument.amount.initialAmount) CC"' 2>/dev/null); \
		if [ -n "$$NEW_AMULETS" ]; then \
			echo "New amulets created:"; \
			echo "$$NEW_AMULETS"; \
		else \
			echo "Transaction completed but no new Amulet contracts found in response."; \
			echo "Run 'make find-amulets' to see your updated amulets."; \
		fi; \
	else \
		echo "❌ Unexpected response format:"; \
		echo "$$TRANSFER_RESP" | jq '.' 2>/dev/null || echo "$$TRANSFER_RESP"; \
		exit 1; \
	fi

merge-amulets: extract-contract-blobs
	@if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ] || [ -z "$(PARTY_APP_USER)" ] || [ -z "$(SPLICE_PACKAGE_ID)" ]; then \
		echo "Error: APP_USER_API_URL, APP_USER_JWT, PARTY_APP_USER, and SPLICE_PACKAGE_ID must be set in .env"; \
		exit 1; \
	fi; \
	if [ ! -f contract-blobs.env ]; then \
		echo "Error: contract-blobs.env not found"; \
		exit 1; \
	fi; \
	source contract-blobs.env; \
	echo "Using contracts from contract-blobs.env:"; \
	echo "  AmuletRules: $$AMULET_RULES_CID"; \
	echo "  OpenMiningRound: $$OPEN_MINING_ROUND_CID"; \
	echo ""; \
	echo "Finding all amulets for $(PARTY_APP_USER)..."; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/state/active-contracts?limit=500" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$(PARTY_APP_USER)" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	AMULETS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("Splice.Amulet:Amulet")) and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("LockedAmulet") | not))]' 2>/dev/null); \
	if [ "$$AMULETS" = "[]" ] || [ -z "$$AMULETS" ] || [ "$$AMULETS" = "null" ]; then \
		echo "No Amulet contracts found for party $(PARTY_APP_USER)"; \
		exit 1; \
	fi; \
	AMULET_COUNT=$$(echo "$$AMULETS" | jq 'length'); \
	echo "Found $$AMULET_COUNT amulet(s)"; \
	if [ "$$AMULET_COUNT" -eq 1 ]; then \
		echo "Only one amulet found, nothing to merge"; \
		exit 0; \
	fi; \
	AMULET_CIDS=$$(echo "$$AMULETS" | jq -r '[.[] | .contractEntry.JsActiveContract.createdEvent.contractId] | join(" ")'); \
	TOTAL_AMOUNT=$$(echo "$$AMULETS" | jq '[.[] | .contractEntry.JsActiveContract.createdEvent.createArgument.amount.initialAmount | tonumber] | add'); \
	echo "Total amount: $$TOTAL_AMOUNT CC"; \
	echo "Merging $$AMULET_COUNT amulets into one..."; \
	echo ""; \
	echo "Building disclosed contracts array..."; \
	DISCLOSED_CONTRACTS="[]"; \
	if [ -n "$$AMULET_RULES_BLOB" ] && [ -n "$$SYNCHRONIZER_ID" ]; then \
		DISCLOSED_CONTRACTS=$$(echo "$$DISCLOSED_CONTRACTS" | jq --arg sync "$$SYNCHRONIZER_ID" '. + [{"templateId": "$(SPLICE_PACKAGE_ID):Splice.AmuletRules:AmuletRules", "contractId": "'"$$AMULET_RULES_CID"'", "createdEventBlob": "'"$$AMULET_RULES_BLOB"'", "synchronizerId": $$sync}]'); \
	fi; \
	if [ -n "$$OPEN_MINING_ROUND_BLOB" ] && [ -n "$$SYNCHRONIZER_ID" ]; then \
		DISCLOSED_CONTRACTS=$$(echo "$$DISCLOSED_CONTRACTS" | jq --arg sync "$$SYNCHRONIZER_ID" '. + [{"templateId": "$(SPLICE_PACKAGE_ID):Splice.Round:OpenMiningRound", "contractId": "'"$$OPEN_MINING_ROUND_CID"'", "createdEventBlob": "'"$$OPEN_MINING_ROUND_BLOB"'", "synchronizerId": $$sync}]'); \
	fi; \
	echo "  Disclosed contracts: $$(echo "$$DISCLOSED_CONTRACTS" | jq 'length') contracts"; \
	INPUT_ARRAY="[]"; \
	for CID in $$AMULET_CIDS; do \
		INPUT_ARRAY=$$(echo "$$INPUT_ARRAY" | jq '. + [{"tag": "InputAmulet", "value": "'$$CID'"}]'); \
	done; \
	TRANSFER_RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/commands/submit-and-wait-for-transaction" \
		-d "$$(jq -nc \
			--arg open_round "$$OPEN_MINING_ROUND_CID" \
			--arg party "$(PARTY_APP_USER)" \
			--arg total "$$TOTAL_AMOUNT" \
			--arg amulet_rules_cid "$$AMULET_RULES_CID" \
			--arg sync_id "$$SYNCHRONIZER_ID" \
			--argjson inputs "$$INPUT_ARRAY" \
			--argjson disclosed "$$DISCLOSED_CONTRACTS" \
			'{commands: {commands: [{ExerciseCommand: {templateId: "$(SPLICE_PACKAGE_ID):Splice.AmuletRules:AmuletRules", contractId: $$amulet_rules_cid, choice: "AmuletRules_Transfer", choiceArgument: {transfer: {sender: $$party, provider: $$party, inputs: $$inputs, outputs: [{receiver: $$party, amount: $$total, receiverFeeRatio: 0.0, lock: null}], beneficiaries: null}, context: {openMiningRound: $$open_round, issuingMiningRounds: [], validatorRights: [], featuredAppRight: null}, expectedDso: "DSO::122075d227a0482dc186fa09a3ddc4e0b2046d1ce9fdf4ec7375bd698b362525632e"}}}], actAs: [$$party], readAs: [], disclosedContracts: $$disclosed, domainId: $$sync_id, workflowId: null, commandId: "merge-\(now)", deduplicationPeriod: null, submissionId: null, minLedgerTimeAbs: null, minLedgerTimeRel: null, applicationId: "hash-merge"}}')"); \
	if echo "$$TRANSFER_RESP" | jq -e '.error' >/dev/null 2>&1; then \
		echo "❌ Error merging amulets:"; \
		echo "$$TRANSFER_RESP" | jq '.' 2>/dev/null || echo "$$TRANSFER_RESP"; \
		exit 1; \
	fi; \
	if echo "$$TRANSFER_RESP" | jq -e '.transaction' >/dev/null 2>&1; then \
		echo "✅ Amulets merged successfully!"; \
		echo ""; \
		MERGED_AMULET=$$(echo "$$TRANSFER_RESP" | jq -r '.transaction.events[]? | select(.CreatedEvent?) | .CreatedEvent | select(.templateId | contains("Splice.Amulet:Amulet")) | "  Contract ID: \(.contractId)\n  Amount: \(.createArgument.amount.initialAmount) CC"' 2>/dev/null); \
		if [ -n "$$MERGED_AMULET" ]; then \
			echo "Merged amulet:"; \
			echo "$$MERGED_AMULET"; \
		else \
			echo "Transaction completed but no merged Amulet contract found in response."; \
			echo "Run 'make find-amulets' to see your updated amulets."; \
		fi; \
	else \
		echo "❌ Unexpected response format:"; \
		echo "$$TRANSFER_RESP" | jq '.' 2>/dev/null || echo "$$TRANSFER_RESP"; \
		exit 1; \
	fi

# TestToken Management
tokens:
	@if [ ! -f rust/mint.env ]; then \
		echo "Error: rust/mint.env not found. Run 'cargo run -- mint' first."; \
		exit 1; \
	fi; \
	source rust/mint.env; \
	if [ -z "$(APP_USER_API_URL)" ] || [ -z "$(APP_USER_JWT)" ]; then \
		echo "Error: APP_USER_API_URL or APP_USER_JWT not set in .env"; \
		exit 1; \
	fi; \
	echo "=== TestToken Balances ==="; \
	echo ""; \
	echo "Package ID: $(HASH_PACKAGE_ID)"; \
	echo "Instrument ID: $$INSTRUMENT_ID"; \
	echo ""; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		$(APP_USER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	echo "Querying holdings via Holding interface at offset $$LEDGER_END..."; \
	INTERFACE_ID="$(HASH_PACKAGE_ID):TestToken:TestToken"; \
	echo ""; \
	echo "Fetching PARTY_HOLDER tokens..."; \
	HOLDER_RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/state/active-contracts" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$(PARTY_HOLDER)" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	HOLDER_TOKENS=$$(echo "$$HOLDER_RESP" | jq --arg inst_id "$$INSTRUMENT_ID" --arg owner "$(PARTY_HOLDER)" --arg template "$$INTERFACE_ID" '[.[] | select((.contractEntry.JsActiveContract.createdEvent.templateId // .createdEvent.templateId) == $$template) | select((.contractEntry.JsActiveContract.createdEvent.createArgument.instrumentId // .createdEvent.createArgument.instrumentId) == $$inst_id) | select((.contractEntry.JsActiveContract.createdEvent.createArgument.owner // .createdEvent.createArgument.owner) == $$owner)]' 2>/dev/null || echo "[]"); \
	HOLDER_COUNT=$$(echo "$$HOLDER_TOKENS" | jq 'length' 2>/dev/null || echo "0"); \
	HOLDER_TOTAL=$$(echo "$$HOLDER_TOKENS" | jq '[.[] | ((.contractEntry.JsActiveContract.createdEvent.createArgument.amount // .createdEvent.createArgument.amount) | tonumber)] | add // 0' 2>/dev/null || echo "0"); \
	echo "✅ PARTY_HOLDER Holdings:"; \
	echo "  Owner: $(PARTY_HOLDER)"; \
	echo "  Token Count: $$HOLDER_COUNT"; \
	echo "  Total Balance: $$HOLDER_TOTAL"; \
	echo "  Instrument: $$INSTRUMENT_ID"; \
	if [ $$HOLDER_COUNT -gt 0 ]; then \
		echo ""; \
		echo "  Contracts:"; \
		echo "$$HOLDER_TOKENS" | jq -r '.[] | "    - CID: " + ((.contractEntry.JsActiveContract.createdEvent.contractId // .createdEvent.contractId)) + " | Amount: " + ((.contractEntry.JsActiveContract.createdEvent.createArgument.amount // .createdEvent.createArgument.amount))'; \
	fi; \
	echo ""; \
	echo "Fetching PARTY_BANK tokens..."; \
	BANK_RESP=$$(curl -s -H "Authorization: Bearer $(APP_USER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_USER_API_URL)v2/state/active-contracts" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$(PARTY_BANK)" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	BANK_TOKENS=$$(echo "$$BANK_RESP" | jq --arg inst_id "$$INSTRUMENT_ID" --arg owner "$(PARTY_BANK)" --arg template "$$INTERFACE_ID" '[.[] | select((.contractEntry.JsActiveContract.createdEvent.templateId // .createdEvent.templateId) == $$template) | select((.contractEntry.JsActiveContract.createdEvent.createArgument.instrumentId // .createdEvent.createArgument.instrumentId) == $$inst_id) | select((.contractEntry.JsActiveContract.createdEvent.createArgument.owner // .createdEvent.createArgument.owner) == $$owner)]' 2>/dev/null || echo "[]"); \
	BANK_COUNT=$$(echo "$$BANK_TOKENS" | jq 'length' 2>/dev/null || echo "0"); \
	BANK_TOTAL=$$(echo "$$BANK_TOKENS" | jq '[.[] | ((.contractEntry.JsActiveContract.createdEvent.createArgument.amount // .createdEvent.createArgument.amount) | tonumber)] | add // 0' 2>/dev/null || echo "0"); \
	echo "✅ PARTY_BANK Holdings:"; \
	echo "  Owner: $(PARTY_BANK)"; \
	echo "  Token Count: $$BANK_COUNT"; \
	echo "  Total Balance: $$BANK_TOTAL"; \
	echo "  Instrument: $$INSTRUMENT_ID"; \
	if [ $$BANK_COUNT -gt 0 ]; then \
		echo ""; \
		echo "  Contracts:"; \
		echo "$$BANK_TOKENS" | jq -r '.[] | "    - CID: " + ((.contractEntry.JsActiveContract.createdEvent.contractId // .createdEvent.contractId)) + " | Amount: " + ((.contractEntry.JsActiveContract.createdEvent.createArgument.amount // .createdEvent.createArgument.amount))'; \
	fi; \
	echo ""; \
	echo "=== Summary ==="; \
	TOTAL=$$(echo "$$HOLDER_TOTAL + $$BANK_TOTAL" | bc); \
	echo "Total Supply: $$TOTAL TestToken"; \
	echo "HOLDER: $$HOLDER_TOTAL ($$HOLDER_COUNT contracts)"; \
	echo "BANK: $$BANK_TOTAL ($$BANK_COUNT contracts)"

# Find FeaturedAppActivityMarkers and AppRewardCoupons for app provider
find-rewards:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi; \
	PARTY=$${PARTY_APP_PROVIDER}; \
	if [ -z "$$PARTY" ]; then \
		echo "Error: PARTY_APP_PROVIDER not set in .env"; \
		exit 1; \
	fi; \
	echo "=== Rewards for Provider: $$PARTY ==="; \
	echo ""; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	echo "Querying active contracts at offset $$LEDGER_END..."; \
	echo ""; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_PROVIDER_API_URL)v2/state/active-contracts?limit=500" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$$PARTY" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	MARKERS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("FeaturedAppActivityMarker")))]' 2>/dev/null); \
	REWARDS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("AppRewardCoupon")))]' 2>/dev/null); \
	echo "## FeaturedAppActivityMarkers"; \
	echo ""; \
	if [ "$$MARKERS" != "[]" ] && [ -n "$$MARKERS" ] && [ "$$MARKERS" != "null" ]; then \
		MARKER_COUNT=$$(echo "$$MARKERS" | jq 'length'); \
		echo "Found $$MARKER_COUNT activity marker(s):"; \
		echo ""; \
		echo "$$MARKERS" | jq -r '.[] | "  Contract ID: \(.contractEntry.JsActiveContract.createdEvent.contractId)\n  Provider: \(.contractEntry.JsActiveContract.createdEvent.createArgument.provider // "N/A")\n  Beneficiary: \(.contractEntry.JsActiveContract.createdEvent.createArgument.beneficiary // "N/A")\n  Weight: \(.contractEntry.JsActiveContract.createdEvent.createArgument.weight // "N/A")\n"' 2>/dev/null; \
		MARKER_CIDS=$$(echo "$$MARKERS" | jq -r '[.[] | .contractEntry.JsActiveContract.createdEvent.contractId] | join(",")' 2>/dev/null); \
		if [ -n "$$MARKER_CIDS" ] && [ "$$MARKER_CIDS" != "null" ]; then \
			echo "FEATURED_APP_ACTIVITY_MARKER_CIDS=$$MARKER_CIDS"; \
			echo ""; \
		fi; \
	else \
		echo "No FeaturedAppActivityMarker contracts found"; \
		echo ""; \
		echo "Activity markers are created when:"; \
		echo "  1. ProveTokenReserves is executed with a FeaturedAppRight"; \
		echo "  2. Featured app transactions occur"; \
		echo ""; \
		echo "These markers are automatically converted to AppRewardCoupons by DSO automation."; \
		echo ""; \
	fi; \
	echo "## AppRewardCoupons"; \
	echo ""; \
	if [ "$$REWARDS" != "[]" ] && [ -n "$$REWARDS" ] && [ "$$REWARDS" != "null" ]; then \
		REWARD_COUNT=$$(echo "$$REWARDS" | jq 'length'); \
		echo "Found $$REWARD_COUNT reward coupon(s):"; \
		echo ""; \
		echo "$$REWARDS" | jq -r '.[] | "  Contract ID: \(.contractEntry.JsActiveContract.createdEvent.contractId)\n  Amount: \(.contractEntry.JsActiveContract.createdEvent.createArgument.amount // "N/A") CC\n  Round: \(.contractEntry.JsActiveContract.createdEvent.createArgument.round.number // "N/A")\n  Featured: \(.contractEntry.JsActiveContract.createdEvent.createArgument.featured // false)\n  Provider: \(.contractEntry.JsActiveContract.createdEvent.createArgument.provider // "N/A")\n  Beneficiary: \(.contractEntry.JsActiveContract.createdEvent.createArgument.beneficiary // "N/A")\n"' 2>/dev/null; \
		COUPON_CIDS=$$(echo "$$REWARDS" | jq -r '[.[] | .contractEntry.JsActiveContract.createdEvent.contractId] | join(",")' 2>/dev/null); \
		if [ -n "$$COUPON_CIDS" ] && [ "$$COUPON_CIDS" != "null" ]; then \
			echo "APP_REWARD_COUPON_CIDS=$$COUPON_CIDS"; \
			echo ""; \
		fi; \
		TOTAL_REWARDS=$$(echo "$$REWARDS" | jq '[.[] | (.contractEntry.JsActiveContract.createdEvent.createArgument.amount // "0" | tonumber)] | add // 0' 2>/dev/null); \
		echo "Total Rewards Available: $$TOTAL_REWARDS CC"; \
		echo ""; \
		echo "To redeem these rewards:"; \
		echo "  1. Wait for an IssuingMiningRound to open"; \
		echo "  2. Exercise the coupon's Mint choice to convert to Canton Coin"; \
		echo ""; \
	else \
		echo "No AppRewardCoupon contracts found"; \
		echo ""; \
		echo "AppRewardCoupons are created by DSO automation from:"; \
		echo "  1. FeaturedAppActivityMarkers (converted periodically)"; \
		echo "  2. Featured app payments and transfers"; \
		echo ""; \
		echo "Check back after activity markers have been converted by the SV automation."; \
		echo ""; \
	fi

# Find FeaturedAppActivityMarkers for app provider (detailed view)
find-markers:
	@if [ -z "$(APP_PROVIDER_API_URL)" ] || [ -z "$(APP_PROVIDER_JWT)" ]; then \
		echo "Error: APP_PROVIDER_API_URL or APP_PROVIDER_JWT not set in .env"; \
		exit 1; \
	fi; \
	PARTY=$${PARTY_APP_PROVIDER}; \
	if [ -z "$$PARTY" ]; then \
		echo "Error: PARTY_APP_PROVIDER not set in .env"; \
		exit 1; \
	fi; \
	echo "=== FeaturedAppActivityMarkers for Provider ==="; \
	echo ""; \
	echo "Provider: $$PARTY"; \
	echo ""; \
	LEDGER_END=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		$(APP_PROVIDER_API_URL)v2/state/ledger-end | jq -r '.offset // empty'); \
	if [ -z "$$LEDGER_END" ] || [ "$$LEDGER_END" = "null" ]; then \
		echo "Error: Unable to get ledger end offset"; \
		exit 1; \
	fi; \
	echo "Querying active contracts at offset $$LEDGER_END..."; \
	echo ""; \
	RESP=$$(curl -s -H "Authorization: Bearer $(APP_PROVIDER_JWT)" \
		-H "Content-Type: application/json" \
		-X POST "$(APP_PROVIDER_API_URL)v2/state/active-contracts?limit=500" \
		-d "$$(jq -nc --arg offset "$$LEDGER_END" --arg party "$$PARTY" \
			'{activeAtOffset: ($$offset | tonumber), filter: {filtersByParty: {($$party): {}}}, verbose: true}')"); \
	MARKERS=$$(echo "$$RESP" | jq '[.[] | select(.contractEntry? and .contractEntry.JsActiveContract? and (.contractEntry.JsActiveContract.createdEvent.templateId | contains("FeaturedAppActivityMarker")))]' 2>/dev/null); \
	if [ "$$MARKERS" != "[]" ] && [ -n "$$MARKERS" ] && [ "$$MARKERS" != "null" ]; then \
		MARKER_COUNT=$$(echo "$$MARKERS" | jq 'length'); \
		echo "Found $$MARKER_COUNT activity marker(s):"; \
		echo ""; \
		echo "$$MARKERS" | jq -r '.[] | "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\nContract ID:\n  \(.contractEntry.JsActiveContract.createdEvent.contractId)\n\nDSO: \(.contractEntry.JsActiveContract.createdEvent.createArgument.dso // "N/A")\nProvider: \(.contractEntry.JsActiveContract.createdEvent.createArgument.provider // "N/A")\nBeneficiary: \(.contractEntry.JsActiveContract.createdEvent.createArgument.beneficiary // "N/A")\nWeight: \(.contractEntry.JsActiveContract.createdEvent.createArgument.weight // "N/A")\n\nTemplate: \(.contractEntry.JsActiveContract.createdEvent.templateId)\nCreated At: \(.contractEntry.JsActiveContract.createdEvent.createdAt // "N/A")\n"' 2>/dev/null; \
		echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; \
		echo ""; \
		MARKER_CIDS=$$(echo "$$MARKERS" | jq -r '[.[] | .contractEntry.JsActiveContract.createdEvent.contractId] | join(",")' 2>/dev/null); \
		if [ -n "$$MARKER_CIDS" ] && [ "$$MARKER_CIDS" != "null" ]; then \
			echo "Export variable for scripting:"; \
			echo "  FEATURED_APP_ACTIVITY_MARKER_CIDS=$$MARKER_CIDS"; \
			echo ""; \
		fi; \
		TOTAL_WEIGHT=$$(echo "$$MARKERS" | jq '[.[] | (.contractEntry.JsActiveContract.createdEvent.createArgument.weight // "0" | tonumber)] | add // 0' 2>/dev/null); \
		echo "Summary:"; \
		echo "  Total Markers: $$MARKER_COUNT"; \
		echo "  Total Weight: $$TOTAL_WEIGHT"; \
		echo ""; \
		echo "These markers will be automatically converted to AppRewardCoupons by DSO automation."; \
		echo "The conversion happens periodically (typically every few rounds)."; \
		echo ""; \
		echo "Once converted, use 'make find-rewards' to see your AppRewardCoupons."; \
	else \
		echo "No FeaturedAppActivityMarker contracts found"; \
		echo ""; \
		echo "Activity markers are created when:"; \
		echo "  1. ProveTokenReserves is executed with a FeaturedAppRight"; \
		echo "  2. Featured app transactions occur with activity tracking"; \
		echo ""; \
		echo "Example: Run 'cargo run -- token-reserve' to create a proof and activity marker."; \
		echo ""; \
		echo "After creation, markers are automatically converted to AppRewardCoupons by"; \
		echo "the DSO automation (usually within a few mining rounds)."; \
		echo ""; \
	fi
