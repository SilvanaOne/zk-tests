# SP1 Add Project Makefile
# This Makefile helps with generating proofs, deploying contracts, and verifying proofs on Sepolia

.PHONY: help build execute prove-core prove-groth16 prove-plonk vkey deploy-groth16 deploy-plonk deploy-sui sui verify-groth16 verify-plonk verify-contract-groth16 verify-contract-plonk check-contract-groth16 check-contract-plonk get-sum-groth16 get-sum-plonk test clean fetch-submodules

# Load environment variables from .env file
include .env
export

# Default target
help:
	@echo "SP1 Add Project Commands:"
	@echo ""
	@echo "Building and Execution:"
	@echo "  build          - Build the SP1 program"
	@echo "  execute        - Execute the program without generating proof"
	@echo "                   Usage: make execute LENGTH=10"
	@echo ""
	@echo "Proof Generation:"
	@echo "  prove-core     - Generate SP1 core proof"
	@echo "                   Usage: make prove-core LENGTH=10"
	@echo "  prove-groth16  - Generate Groth16 EVM proof and fixture"
	@echo "                   Usage: make prove-groth16 LENGTH=10"
	@echo "  prove-plonk    - Generate PLONK EVM proof and fixture"
	@echo "                   Usage: make prove-plonk LENGTH=10"
	@echo "  vkey          - Get the program verification key"
	@echo ""
	@echo "Contract Deployment:"
	@echo "  deploy-groth16 - Deploy Add contract with Groth16 verifier"
	@echo "  deploy-plonk   - Deploy Add contract with PLONK verifier"
	@echo "  deploy-sui     - Build and deploy Sui Move contract"
	@echo ""
	@echo "Sui Client:"
	@echo "  sui            - Run Sui client application"
	@echo ""
	@echo "Proof Verification on Sepolia:"
	@echo "  verify-groth16 - Verify Groth16 proof on deployed contract"
	@echo "  verify-plonk   - Verify PLONK proof on deployed contract"
	@echo ""
	@echo "Contract Verification & Status:"
	@echo "  verify-contract-groth16 - Verify Groth16 contract on Etherscan"
	@echo "  verify-contract-plonk   - Verify PLONK contract on Etherscan"
	@echo "  check-contract-groth16  - Check Groth16 contract status & info"
	@echo "  check-contract-plonk    - Check PLONK contract status & info"
	@echo "  get-sum-groth16         - Get current sum from Groth16 contract"
	@echo "  get-sum-plonk           - Get current sum from PLONK contract"
	@echo ""
	@echo "Testing:"
	@echo "  test           - Run Foundry tests"
	@echo ""
	@echo "Utilities:"
	@echo "  fetch-submodules - Fetch and initialize all Git submodules"
	@echo "  clean          - Clean build artifacts"

# Building and execution
build:
	@echo "Building SP1 program..."
	cd script && cargo build --release

execute:
	@echo "Executing program without proof..."
	@echo "Usage: make execute LENGTH=10"
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release -- --execute --length $(or $(LENGTH),10); \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

# Proof generation
prove-core:
	@echo "Generating SP1 core proof..."
	@echo "Usage: make prove-core LENGTH=10"
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release -- --prove --length $(or $(LENGTH),10); \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

prove-groth16:
	@echo "Generating Groth16 EVM proof..."
	@echo "Usage: make prove-groth16 LENGTH=10"
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release --bin evm -- --system groth16 --length $(or $(LENGTH),10); \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

prove-plonk:
	@echo "Generating PLONK EVM proof..."
	@echo "Usage: make prove-plonk LENGTH=10"
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release --bin evm -- --system plonk --length $(or $(LENGTH),10); \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

vkey:
	@echo "Getting program verification key..."
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release --bin vkey; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

# Contract deployment
deploy-groth16:
	@echo "Deploying Add contract with Groth16 verifier..."
	@echo "Verifier: $(VERIFIER_GROTH16)"
	@echo "Program VKey: $(PROGRAM_VKEY)"
	cd ethereum && forge create src/Add.sol:Add \
		--rpc-url $(RPC_URL) \
		--private-key $(PRIVATE_KEY) \
		--broadcast \
		--constructor-args $(VERIFIER_GROTH16) $(PROGRAM_VKEY) \
		--verify \
		--verifier etherscan \
		--etherscan-api-key $(ETHERSCAN_API_KEY)

deploy-plonk:
	@echo "Deploying Add contract with PLONK verifier..."
	@echo "Verifier: $(VERIFIER_PLONK)"
	@echo "Program VKey: $(PROGRAM_VKEY)"
	cd ethereum && forge create src/Add.sol:Add \
		--rpc-url $(RPC_URL) \
		--private-key $(PRIVATE_KEY) \
		--broadcast \
		--constructor-args $(VERIFIER_PLONK) $(PROGRAM_VKEY) \
		--verify \
		--verifier etherscan \
		--etherscan-api-key $(ETHERSCAN_API_KEY)

deploy-sui:
	@echo "Building and deploying Sui Move contract..."
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd sui/move && \
	echo "Building Sui Move contract..." && \
	sui move build && \
	echo "Deploying Sui Move contract..." && \
	sui client publish; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

sui:
	@echo "Running Sui client application..."
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd sui/client && \
	cargo run; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

# Proof verification on Sepolia
verify-groth16:
	@echo "Verifying Groth16 proof on Sepolia..."
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release --bin verify -- --proof-type groth16; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

verify-plonk:
	@echo "Verifying PLONK proof on Sepolia..."
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd script && cargo run --release --bin verify -- --proof-type plonk; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

# Contract verification on Sepolia
verify-contract-groth16:
	@echo "Verifying Groth16 contract deployment on Sepolia..."
	@echo "Contract Address: $(CONTRACT_ADDRESS_GROTH16)"
	@if [ -z "$(CONTRACT_ADDRESS_GROTH16)" ]; then \
		echo "❌ CONTRACT_ADDRESS_GROTH16 not set in .env"; \
		exit 1; \
	fi
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd ethereum && forge verify-contract \
		--rpc-url $(RPC_URL) \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		--constructor-args $(shell cast abi-encode "constructor(address,bytes32)" $(VERIFIER_GROTH16) $(PROGRAM_VKEY)) \
		$(CONTRACT_ADDRESS_GROTH16) \
		src/Add.sol:Add; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

verify-contract-plonk:
	@echo "Verifying PLONK contract deployment on Sepolia..."
	@echo "Contract Address: $(CONTRACT_ADDRESS_PLONK)"
	@if [ -z "$(CONTRACT_ADDRESS_PLONK)" ]; then \
		echo "❌ CONTRACT_ADDRESS_PLONK not set in .env"; \
		exit 1; \
	fi
	@echo "⏱️  Start time: $$(date '+%Y-%m-%d %H:%M:%S')"
	@start_time=$$(date +%s); \
	cd ethereum && forge verify-contract \
		--rpc-url $(RPC_URL) \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		--constructor-args $(shell cast abi-encode "constructor(address,bytes32)" $(VERIFIER_PLONK) $(PROGRAM_VKEY)) \
		$(CONTRACT_ADDRESS_PLONK) \
		src/Add.sol:Add; \
	end_time=$$(date +%s); \
	duration=$$((end_time - start_time)); \
	echo "⏱️  End time: $$(date '+%Y-%m-%d %H:%M:%S')"; \
	echo "⏱️  Duration: $${duration} seconds"

# Contract status checking
check-contract-groth16:
	@echo "Checking Groth16 contract status on Sepolia..."
	@echo "Contract Address: $(CONTRACT_ADDRESS_GROTH16)"
	@if [ -z "$(CONTRACT_ADDRESS_GROTH16)" ]; then \
		echo "❌ CONTRACT_ADDRESS_GROTH16 not set in .env"; \
		exit 1; \
	fi
	@echo "Current Sum:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_GROTH16) "getCurrentSum()" --rpc-url $(RPC_URL)
	@echo "Verifier Address:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_GROTH16) "verifier()" --rpc-url $(RPC_URL)
	@echo "Program VKey:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_GROTH16) "addProgramVKey()" --rpc-url $(RPC_URL)

check-contract-plonk:
	@echo "Checking PLONK contract status on Sepolia..."
	@echo "Contract Address: $(CONTRACT_ADDRESS_PLONK)"
	@if [ -z "$(CONTRACT_ADDRESS_PLONK)" ]; then \
		echo "❌ CONTRACT_ADDRESS_PLONK not set in .env"; \
		exit 1; \
	fi
	@echo "Current Sum:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_PLONK) "getCurrentSum()" --rpc-url $(RPC_URL)
	@echo "Verifier Address:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_PLONK) "verifier()" --rpc-url $(RPC_URL)
	@echo "Program VKey:"
	cd ethereum && cast call $(CONTRACT_ADDRESS_PLONK) "addProgramVKey()" --rpc-url $(RPC_URL)

# Contract interaction
get-sum-groth16:
	@echo "Getting current sum from Groth16 contract..."
	@if [ -z "$(CONTRACT_ADDRESS_GROTH16)" ]; then \
		echo "❌ CONTRACT_ADDRESS_GROTH16 not set in .env"; \
		exit 1; \
	fi
	cd ethereum && cast call $(CONTRACT_ADDRESS_GROTH16) "getCurrentSum()" --rpc-url $(RPC_URL) | cast --to-dec

get-sum-plonk:
	@echo "Getting current sum from PLONK contract..."
	@if [ -z "$(CONTRACT_ADDRESS_PLONK)" ]; then \
		echo "❌ CONTRACT_ADDRESS_PLONK not set in .env"; \
		exit 1; \
	fi
	cd ethereum && cast call $(CONTRACT_ADDRESS_PLONK) "getCurrentSum()" --rpc-url $(RPC_URL) | cast --to-dec

# Testing
test:
	@echo "Running Foundry tests..."
	cd ethereum && forge test -v

# Utilities
fetch-submodules:
	@echo "Fetching and initializing Git submodules..."
	@echo "Registering submodules from ethereum/.gitmodules..."
	@cd ethereum && \
		git submodule add --force https://github.com/foundry-rs/forge-std lib/forge-std && \
		git submodule add --force https://github.com/succinctlabs/sp1-contracts lib/sp1-contracts && \
		echo "Updating submodules to correct tags..." && \
		cd lib/forge-std && git checkout v1.8.2 && \
		cd ../sp1-contracts && git checkout v3.0.0
	@echo "✅ Submodules fetched and configured successfully!"

clean:
	@echo "Cleaning build artifacts..."
	cd script && cargo clean
	cd ethereum && forge clean

# Full workflow targets
full-groth16: prove-groth16 deploy-groth16 verify-groth16
	@echo "✅ Complete Groth16 workflow finished!"

full-plonk: prove-plonk deploy-plonk verify-plonk
	@echo "✅ Complete PLONK workflow finished!"

# Development targets
dev-groth16: prove-groth16 verify-groth16
	@echo "✅ Groth16 development cycle completed!"

dev-plonk: prove-plonk verify-plonk
	@echo "✅ PLONK development cycle completed!"