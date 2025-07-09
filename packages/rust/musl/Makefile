# Application name and target
APP_NAME := app
TARGET   := aarch64-unknown-linux-musl
IMAGE    := $(APP_NAME):musl

.PHONY: all build docker clean stats run

# Default target: build the Rust binary
all: docker

# Build the Rust binary for musl
build:
	@echo "Adding musl target and building..."
	@rustup target add $(TARGET)
	@cargo build --release --target $(TARGET)

# Build the Docker image using the multi-stage Dockerfile
docker:
	@echo "Building Docker image $(IMAGE)..."
	@docker build -t $(IMAGE) .

# Clean build artifacts
clean:
	@echo "Cleaning up..."
	@cargo clean

# Print Docker image stats: size, layers, architecture
stats:
	@echo "Docker image: $(IMAGE)"
	@docker image inspect $(IMAGE) --format 'Size: {{.Size}} bytes'
	@docker image inspect $(IMAGE) --format 'Architecture: {{.Architecture}}'
	@docker image inspect $(IMAGE) --format 'Layers: {{len .RootFS.Layers}}'


# Run the container locally and capture peak memory consumption
run:
	@CID=$$(docker run -d --name $(APP_NAME)_memtest --rm $(IMAGE)); \
	MAX=0; \
	while docker ps -q --no-trunc | grep -q $$CID; do \
	    CUR=$$(docker stats --no-stream --format '{{.MemUsage}}' $$CID | awk '{print $$1}'); \
	    UNIT=$${CUR##*[0-9.]}; \
	    VAL=$${CUR%$$UNIT}; \
	    case $$UNIT in \
	        KiB) BYTES=$$(echo "($$VAL)*1024" | bc);; \
	        MiB) BYTES=$$(echo "($$VAL)*1048576" | bc);; \
	        GiB) BYTES=$$(echo "($$VAL)*1073741824" | bc);; \
	        *)   BYTES=$$(echo "$$VAL" | bc);; \
	    esac; \
	    COMP=$$(echo "$$BYTES > $$MAX" | bc); \
	    if [ $$COMP -eq 1 ]; then MAX=$$BYTES; fi; \
	    sleep 1; \
	done; \
	echo "Peak memory usage for $(IMAGE): $${MAX} bytes"; \
	docker rm -f $$CID >/dev/null 2>&1 || true
