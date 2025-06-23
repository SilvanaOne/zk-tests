# ------------------------------------------------------------
# Makefile for building and running the tee-arm Nitro Enclave
# ------------------------------------------------------------

# Tunables ----------------------------------------------------
IMAGE_NAME ?= tee-arm:latest
OUT_DIR ?= out
EIF_NAME ?= $(OUT_DIR)/tee-arm.eif
CPU_COUNT ?= 2
MEMORY ?= 512 # MiB
ENCLAVE_CID ?= 16

# Default target ------------------------------------------------
.PHONY: all
all: $(EIF_NAME)

# Build the container image ------------------------------------
.PHONY: image
image:
	docker buildx build --platform linux/arm64 -t $(IMAGE_NAME) -f Containerfile .

# Create the Enclave Image File (EIF) ---------------------------
$(EIF_NAME): image
	mkdir -p $(OUT_DIR)
	nitro-cli build-enclave \
		--docker-uri $(IMAGE_NAME) \
		--output-file $(EIF_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY)

# Create the EIF with debug symbols enabled --------------------
.PHONY: eif-debug
EIF_DEBUG_NAME := $(OUT_DIR)/tee-arm-debug.eif
eif-debug: image
	mkdir -p $(OUT_DIR)
	nitro-cli build-enclave \
		--docker-uri $(IMAGE_NAME) \
		--output-file $(EIF_DEBUG_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--debug-mode

# Run the enclave ----------------------------------------------
.PHONY: run
run: $(EIF_NAME)
	nitro-cli run-enclave \
		--eif-path $(EIF_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID)

# Run the enclave with debug mode enabled ----------------------
.PHONY: run-debug
run-debug: eif-debug
	nitro-cli run-enclave \
		--eif-path $(EIF_DEBUG_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID) \
		--debug-mode

# Run the enclave with debug mode enabled and attach console ----------------------
.PHONY: run-debug-console
run-debug-console: eif-debug
	nitro-cli run-enclave \
		--eif-path $(EIF_DEBUG_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID) \
		--debug-mode \
		--attach-console

# Clean targets -------------------------------------------------
.PHONY: clean
clean:
	rm -rf $(OUT_DIR) 