# ------------------------------------------------------------
# Makefile for building and running the tee-arm Nitro Enclave
# ------------------------------------------------------------

# Tunables ----------------------------------------------------
IMAGE_NAME := tee-arm:latest
NITRO_BUILDER_IMAGE := tee-arm-nitro-builder:latest
OUT_DIR := out
EIF_NAME := $(OUT_DIR)/tee-arm.eif
CPU_COUNT := 1
MEMORY := 3072 # MiB
ENCLAVE_CID := 16

# Default target ------------------------------------------------
.DEFAULT_GOAL := build
.PHONY: default
default: $(EIF_NAME)

# Build target for full reproducible build ---------------------
.PHONY: build
build: $(EIF_NAME)

# Build the container image ------------------------------------
.PHONY: image
image:
	mkdir -p $(OUT_DIR)
	docker buildx build --platform linux/arm64 --output type=docker -t $(IMAGE_NAME) --target runtime -f Containerfile .

# Build the nitro-cli builder image ----------------------------
.PHONY: nitro-builder-image
nitro-builder-image:
	docker buildx build --platform linux/arm64 --output type=docker -t $(NITRO_BUILDER_IMAGE) --target nitro-builder -f Containerfile .

# Create the Enclave Image File (EIF) using Docker -------------
$(EIF_NAME): image nitro-builder-image
	mkdir -p $(OUT_DIR)
	@echo "Building EIF with nitro-cli (using image present in host daemon)..."
	docker run --rm \
		-v /var/run/docker.sock:/var/run/docker.sock \
		-v $(PWD)/$(OUT_DIR):/workspace/out \
		--platform linux/arm64 \
		$(NITRO_BUILDER_IMAGE) \
		sh -c "mkdir -p /var/log/nitro_enclaves && \
		echo \"CPIO size: \$$(stat -c%s /app/rootfs.cpio)\" && \
		eif_build \
			--kernel /Image \
			--kernel_config /Image.config \
			--ramdisk /app/rootfs.cpio \
			--output /workspace/out/tee-arm.eif \
			--cmdline \"init=/init reboot=k initrd=0x2000000,\$$(stat -c%s /app/rootfs.cpio) root=/dev/ram0 panic=1 pci=off nomodules console=ttyS0 i8042.noaux i8042.nomux i8042.nopnp i8042.dumbkbd\""
		      #  nitro-cli build-enclave \
		      #      --docker-uri $(IMAGE_NAME) \
		      #      --output-file /workspace/out/tee-arm.eif

.PHONY: eif
eif: $(EIF_NAME)

# Run the enclave ----------------------------------------------
.PHONY: run
run:
	sudo nitro-cli run-enclave \
		--eif-path $(EIF_NAME) \
		--cpu-count $(CPU_COUNT) \
		--memory $(MEMORY) \
		--enclave-cid $(ENCLAVE_CID)

# Run the enclave with debug mode enabled ----------------------
.PHONY: run-debug
run-debug:
	sudo nitro-cli run-enclave \
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

# Docker-based nitro-cli operations ----------------------------
.PHONY: nitro-version
nitro-version: nitro-builder-image
	docker run --rm $(NITRO_BUILDER_IMAGE) nitro-cli --version

.PHONY: nitro-help
nitro-help: nitro-builder-image
	docker run --rm $(NITRO_BUILDER_IMAGE) nitro-cli --help

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

# Start vsock-proxy for AWS services -----------------------------
.PHONY: vsock
vsock:
	@echo "Starting vsock-proxy..."
	vsock-proxy 8101 dynamodb.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
	vsock-proxy 8102 kms.us-east-1.amazonaws.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
	vsock-proxy 8103 www.googleapis.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
	vsock-proxy 8104 api.github.com 443 --config /etc/nitro_enclaves/vsock-proxy.yaml &
	@echo "Vsock-proxy started"

# Clean targets -------------------------------------------------
.PHONY: clean
clean:
	rm -rf $(OUT_DIR)
	docker rmi $(IMAGE_NAME) $(NITRO_BUILDER_IMAGE) 2>/dev/null || true

.PHONY: clean-images
clean-images:
	docker rmi $(IMAGE_NAME) $(NITRO_BUILDER_IMAGE) 2>/dev/null || true

.PHONY: update
update:
	git pull origin main && rm -rf $(OUT_DIR)

.PHONY: upgrade
upgrade:
	git pull origin main && rm -rf $(OUT_DIR)
	$(MAKE) build