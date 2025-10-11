# Simple Makefile for building x86_64 binary

BINARY_NAME = billing
X86_DIR = target/x86_64-unknown-linux-gnu/release

.PHONY: build-x86

build-x86: ## Build x86_64 Linux binary using Docker buildx cloud builder
	@echo "Building $(BINARY_NAME) for x86_64 Linux..."
	@mkdir -p $(X86_DIR)

	# Build and extract binary using cloud builder
	docker buildx build \
		--builder cloud-dfstio-silvana \
		--platform linux/amd64 \
		-f Dockerfile.x86 \
		--target export \
		--output type=local,dest=./build-output \
		.

	# Move the binary to the target directory
	@mkdir -p $(X86_DIR)
	@mv ./build-output/billing $(X86_DIR)/$(BINARY_NAME)
	@rm -rf ./build-output

	@echo "✓ Binary built at $(X86_DIR)/$(BINARY_NAME)"

copy: ## Copy built binary, config files to x86 directory
	@echo "Copying files to x86..."
	@mkdir -p x86
	cp $(X86_DIR)/$(BINARY_NAME) x86/
	cp subscriptions.json x86/
	cp users.json x86/
	cp .env x86/
	@echo "✓ Copied to x86/"
	@echo "  - $(BINARY_NAME)"
	@echo "  - subscriptions.json"
	@echo "  - users.json"
	@echo "  - .env"
