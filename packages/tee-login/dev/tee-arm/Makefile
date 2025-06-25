# ------------------------------------------------------------
# Makefile for building and running the tee-arm Nitro Enclave
# ------------------------------------------------------------

# Tunables ----------------------------------------------------
IMAGE_NAME ?= tee-arm:latest
# freeze build timestamp at a reproducible point (2025-06-25 22:09:51 UTC)
SOURCE_DATE_EPOCH ?= 1750889391
OUT_DIR ?= out
EIF_NAME ?= $(OUT_DIR)/tee-arm.eif
CPU_COUNT ?= 1
MEMORY ?= 512 # MiB
ENCLAVE_CID ?= 16
VENDOR_DIR ?= vendor

# Runtime stage image -------------------------------------------------
RUNTIME_IMAGE ?= tee-arm-runtime:local

# CLI image
CLI_IMAGE ?= nitro-cli:1.4.2

# Default target ------------------------------------------------
.DEFAULT_GOAL := $(EIF_NAME)
.PHONY: default
default: $(EIF_NAME)

# Build the container image ------------------------------------
.PHONY: image
image:
	mkdir -p out
	docker buildx build --platform linux/arm64 --build-arg SOURCE_DATE_EPOCH=$(SOURCE_DATE_EPOCH) -t $(IMAGE_NAME) -f Containerfile .

# Build the container image without cache ---------------------
.PHONY: image-no-cache
image-no-cache:
	mkdir -p out
	docker buildx build --platform linux/arm64 --no-cache --build-arg SOURCE_DATE_EPOCH=$(SOURCE_DATE_EPOCH) -t $(IMAGE_NAME) -f Containerfile .

# Build pinned nitro-cli container ------------------------------------
.PHONY: cli-image
cli-image:
	docker buildx build --platform linux/arm64 --build-arg SOURCE_DATE_EPOCH=$(SOURCE_DATE_EPOCH) -f cli.Dockerfile -t $(CLI_IMAGE) .

# Create the Enclave Image File (EIF) --------------------------------
$(EIF_NAME): runtime-image cli-image
	mkdir -p $(OUT_DIR)
	docker run --rm --privileged \
		-v /var/run/docker.sock:/var/run/docker.sock \
		-v $(PWD)/$(OUT_DIR):/out \
		$(CLI_IMAGE) build-enclave \
			--docker-uri $(RUNTIME_IMAGE) \
			--output-file /out/$(notdir $(EIF_NAME))

.PHONY: eif
eif: $(EIF_NAME)

# Run the enclave ----------------------------------------------
.PHONY: run
run:
	nitro-cli run-enclave \
		--eif-path $(EIF_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID)

# Run the enclave with debug mode enabled ----------------------
.PHONY: run-debug
run-debug:
	nitro-cli run-enclave \
		--eif-path $(EIF_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID) \
		--debug-mode \
		--attach-console


# List running enclaves -----------------------------------------
.PHONY: list
list:
	nitro-cli describe-enclaves

# Stop all enclaves and cleanup ---------------------------------
.PHONY: stop
stop:
	sudo nitro-cli terminate-enclave --all
	sudo killall socat || true

# Expose enclave ports via socat --------------------------------
.PHONY: expose
expose:
	@ENCLAVE_ID=$$(nitro-cli describe-enclaves | jq -r ".[0].EnclaveID"); \
	ENCLAVE_CID=$$(nitro-cli describe-enclaves | jq -r ".[0].EnclaveCID"); \
	echo "Enclave ID: $$ENCLAVE_ID"; \
	echo "Enclave CID: $$ENCLAVE_CID"; \
	echo "Sending env.json to enclave..."; \
	cat env.json | socat - VSOCK-CONNECT:$$ENCLAVE_CID:7777; \
	echo "Setting up port forwarding: localhost:3000 -> enclave:3000"; \
	socat TCP4-LISTEN:3000,reuseaddr,fork VSOCK-CONNECT:$$ENCLAVE_CID:3000 &

# Clean targets -------------------------------------------------
.PHONY: clean
clean:
	rm -rf $(OUT_DIR) 

.PHONY: update
update:
	cd ../../../.. && git pull origin main && cd packages/tee-login/dev/tee-arm && rm -rf out
	$(MAKE) image
	$(MAKE) eif

.PHONY: vendor
vendor:
	cd server && cargo vendor $(VENDOR_DIR)

.PHONY: runtime-image
runtime-image:
	docker buildx build --platform linux/arm64 --target runtime --build-arg SOURCE_DATE_EPOCH=$(SOURCE_DATE_EPOCH) -t $(RUNTIME_IMAGE) -f Containerfile .