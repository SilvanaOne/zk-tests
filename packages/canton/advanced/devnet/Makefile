.PHONY: help build run test clean openapi-validate openapi-generate openapi-generate-rust

# Default target
help:
	@echo "Available targets:"
	@echo "  build              - Build the Rust project"
	@echo "  run                - Run the devnet server"
	@echo "  test               - Run tests"
	@echo "  clean              - Clean build artifacts"
	@echo "  openapi-validate   - Validate OpenAPI specification"
	@echo "  openapi-generate   - Generate code from OpenAPI spec"
	@echo "  openapi-generate-rust - Generate Rust models from OpenAPI spec"

# Build
build:
	cargo build --release

# Run server
run:
	cargo run -- start

# Test
test:
	cargo test

# Clean
clean:
	cargo clean
	rm -rf api-generated/src/models/*.rs
	rm -rf api-generated/src/apis/*.rs

# OpenAPI validation
openapi-validate:
	@echo "Validating OpenAPI specification..."
	openapi-generator validate -i openapi.yaml

# OpenAPI code generation (all targets)
openapi-generate: openapi-validate openapi-generate-rust

# Generate Rust models from OpenAPI spec
openapi-generate-rust: openapi-validate
	@echo "Generating Rust code from OpenAPI spec..."
	openapi-generator generate \
		-i openapi.yaml \
		-g rust \
		-o ./api-generated \
		--additional-properties=packageName=api_generated,packageVersion=0.1.0 \
		--global-property=models,supportingFiles
	@echo "Rust code generation complete. Generated files in ./api-generated/"
