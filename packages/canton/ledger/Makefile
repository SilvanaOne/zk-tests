# Simple Makefile for building x86_64 binary

BINARY_NAME = scan
X86_DIR = target/x86_64-unknown-linux-gnu/release

.PHONY: build-x86

build-x86: ## Build x86_64 Linux binary using Docker buildx
	@echo "Building $(BINARY_NAME) for x86_64 Linux..."
	@mkdir -p $(X86_DIR)
	
	# Build and extract binary
	docker buildx build \
		--platform linux/amd64 \
		-f Dockerfile.x86 \
		--target builder \
		-t scan-x86-builder \
		--load \
		.
	
	# Extract the binary
	docker create --name scan-extract scan-x86-builder
	docker cp scan-extract:/scan $(X86_DIR)/$(BINARY_NAME)
	docker rm scan-extract
	docker rmi scan-x86-builder
	
	@echo "✓ Binary built at $(X86_DIR)/$(BINARY_NAME)"

copy: ## Copy built binary, .env.devnet, and Makefile.x86 to canton/scan directory
	@echo "Copying files to canton/scan..."
	@mkdir -p ../../../../../canton/scan
	cp $(X86_DIR)/$(BINARY_NAME) ../../../../canton/scan/
	cp .env.devnet ../../../../canton/scan/.env
	cp Makefile.x86 ../../../../canton/scan/Makefile
	@echo "✓ Copied to ../../../../canton/scan/"
	@echo "  - $(BINARY_NAME)"
	@echo "  - .env"
	@echo "  - Makefile"
