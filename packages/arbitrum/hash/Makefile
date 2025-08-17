faucet:
	cast send 0xbc356b91e24e0f3809fd1E455fc974995eF124dF \
		--value 100ether \
		--private-key 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a \
		--rpc-url http://127.0.0.1:8545

balance:
	cast balance 0xbc356b91e24e0f3809fd1E455fc974995eF124dF \
		--rpc-url http://127.0.0.1:8545

deploy-local:
	forge script script/Deploy.s.sol:Deploy \
		--rpc-url http://127.0.0.1:8545 \
		--broadcast

deploy-hash:
	forge script script/DeployHash.s.sol:DeployHash \
		--rpc-url http://127.0.0.1:8545 \
		--broadcast

deploy-poseidon:
	forge script script/DeployPoseidon.s.sol:DeployPoseidon \
		--rpc-url http://127.0.0.1:8545 \
		--broadcast

deploy-arbitrum:
	@set -a; source .env; set +a; \
	forge script script/DeployPoseidon.s.sol:DeployPoseidon \
		--rpc-url $$ARBITRUM_SEPOLIA_RPC \
		--broadcast

verify:
	@set -a; source .env; set +a; \
	forge verify-contract \
		0xA399434897F2D8D7Fbf5BD032b1e31b3f0E16666 \
		src/Poseidon.sol:Poseidon \
		--chain-id 421614 \
		--etherscan-api-key $$ARBISCAN_API_KEY

test-local:
	@echo "Calling Poseidon setHash function with [1,2,3]..."
	@set -a; source .env; set +a; \
	cast send 0x1d352FCeE947706A68B9b0dAB0d35919Ed04BFf9 \
		"setHash(uint256[])" \
		"[1,2,3]" \
		--private-key $$PRIVATE_KEY \
		--rpc-url http://127.0.0.1:8545; \
	echo "Reading hash result..."; \
	cast call 0x1d352FCeE947706A68B9b0dAB0d35919Ed04BFf9 \
		"hash()" \
		--rpc-url http://127.0.0.1:8545

.PHONY: faucet balance deploy-local deploy-hash deploy-poseidon deploy-arbitrum verify test-local