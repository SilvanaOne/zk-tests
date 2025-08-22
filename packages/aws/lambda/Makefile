.PHONY: help
help: ## Show this help message
	@echo "AWS Lambda Rust Calculator - Available Commands"
	@echo "==============================================="
	@echo ""
	@echo "Usage: make [command]"
	@echo ""
	@echo "Build Commands:"
	@echo "  build              Build all Rust crates"
	@echo "  lambda             Build Lambda deployment package (ARM64)"
	@echo ""
	@echo "Deployment Commands:"
	@echo "  deploy             Build and deploy to AWS (runs 'make lambda' first)"
	@echo "  preview            Preview Pulumi infrastructure changes"
	@echo ""
	@echo "OpenAPI Commands:"
	@echo "  openapi-validate   Validate OpenAPI specification"
	@echo "  openapi-generate   Generate Rust models from OpenAPI spec"
	@echo ""
	@echo "Testing Commands:"
	@echo "  test               Test the deployed /add endpoint with a=2, b=3"
	@echo "  test-api           Test via API Gateway"
	@echo "  test-add           Test /add with a=100, b=200"
	@echo "  test-multiply      Test /multiply with a=10, b=20"
	@echo ""
	@echo "Secrets Management:"
	@echo "  add-secrets        Add Sui blockchain secrets to Pulumi"
	@echo "  list-secrets       List configured Pulumi secrets"
	@echo "  remove-secrets     Remove Sui secrets from Pulumi"
	@echo ""
	@echo "For detailed documentation, see README.md"
	@echo ""

.DEFAULT_GOAL := help

build: ## Build all Rust crates
	cargo build

lambda:
	ulimit -n 10240 && \
	cargo lambda build --release --arm64 --bin bootstrap -p lambda

deploy: lambda
	cd pulumi && pulumi up

preview:
	cd pulumi && pulumi preview

test:
	@echo "Testing Lambda function URL with add endpoint..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/add \
		-H "Content-Type: application/json" \
		-d '{"a": 2, "b": 3}' \
		| python3 -m json.tool

test-api:
	@echo "Testing Lambda via API Gateway with add endpoint..."
	@curl -X POST https://wprchl5wjd.execute-api.us-east-1.amazonaws.com/stage/add \
		-H "Content-Type: application/json" \
		-d '{"a": 2, "b": 3}' \
		| python3 -m json.tool

test-client: 
	@echo "Testing TypeScript client..."
	cd client && npm run test

openapi-generate: openapi-validate openapi-generate-rust openapi-generate-ts

openapi-generate-rust: openapi-validate
	@echo "Generating Rust code from OpenAPI spec..."
	openapi-generator generate \
		-i api/openapi.yaml \
		-g rust \
		-o ./crates/api-generated \
		--additional-properties=packageName=api_generated,packageVersion=0.1.0 \
		--global-property=models,supportingFiles

openapi-generate-ts: openapi-validate
	@echo "Generating TypeScript client from OpenAPI spec..."
	cd client && npx openapi-ts

openapi-validate:
	@echo "Validating OpenAPI specification..."
	openapi-generator validate -i api/openapi.yaml

test-add:
	@echo "Testing add endpoint..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/add \
		-H "Content-Type: application/json" \
		-d '{"a": 100, "b": 200}' \
		| python3 -m json.tool

test-multiply:
	@echo "Testing multiply endpoint..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/multiply \
		-H "Content-Type: application/json" \
		-d '{"a": 10, "b": 20}' \
		| python3 -m json.tool

test-generate-keypair:
	@echo "Testing generate Sui keypair endpoint..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/generate-sui-keypair \
		-H "Content-Type: application/json" \
		-d '{"login_type": "email", "login": "test@example.com"}' \
		| python3 -m json.tool

test-sign-message:
	@echo "Testing sign message endpoint..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/sign-message \
		-H "Content-Type: application/json" \
		-d '{"login_type": "email", "login": "test@example.com", "message": [72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100]}' \
		| python3 -m json.tool

# Pulumi secrets management
add-secrets:
	@echo "Adding Sui secrets to Pulumi..."
	@if [ -f .env ]; then \
		export $$(cat .env | xargs) && \
		cd pulumi && \
		pulumi config set --secret suiPackageId "$$SUI_ADD_PACKAGE_ID" && \
		pulumi config set --secret suiChain "$$SUI_CHAIN" && \
		pulumi config set --secret suiAddress "$$SUI_ADDRESS" && \
		pulumi config set --secret suiSecretKey "$$SUI_SECRET_KEY" && \
		pulumi config set --secret silvanaRegistryPackageDevnet "$$SILVANA_REGISTRY_PACKAGE_DEVNET" && \
		pulumi config set --secret silvanaRegistryPackageTestnet "$$SILVANA_REGISTRY_PACKAGE_TESTNET" && \
		echo "Secrets added successfully"; \
	else \
		echo "Error: .env file not found"; \
		exit 1; \
	fi

list-secrets:
	@echo "Current Pulumi secrets:"
	@cd pulumi && echo -n "suiPackageId: " && (pulumi config get suiPackageId 2>/dev/null || echo "not set")
	@cd pulumi && echo -n "suiChain: " && (pulumi config get suiChain 2>/dev/null || echo "not set")
	@cd pulumi && echo -n "suiAddress: " && (pulumi config get suiAddress 2>/dev/null || echo "not set")
	@cd pulumi && echo -n "suiSecretKey: " && (pulumi config get suiSecretKey 2>/dev/null | sed 's/suiprivkey.*/[HIDDEN]/' || echo "not set")
	@cd pulumi && echo -n "silvanaRegistryPackageDevnet: " && (pulumi config get silvanaRegistryPackageDevnet 2>/dev/null || echo "not set")
	@cd pulumi && echo -n "silvanaRegistryPackageTestnet: " && (pulumi config get silvanaRegistryPackageTestnet 2>/dev/null || echo "not set")

remove-secrets:
	@echo "Removing Sui secrets from Pulumi..."
	@cd pulumi && \
	pulumi config rm suiPackageId 2>/dev/null || true && \
	pulumi config rm suiChain 2>/dev/null || true && \
	pulumi config rm suiAddress 2>/dev/null || true && \
	pulumi config rm suiSecretKey 2>/dev/null || true && \
	pulumi config rm silvanaRegistryPackageDevnet 2>/dev/null || true && \
	pulumi config rm silvanaRegistryPackageTestnet 2>/dev/null || true && \
	echo "Secrets removed"