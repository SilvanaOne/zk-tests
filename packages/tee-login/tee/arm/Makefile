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
.DEFAULT_GOAL :=
.PHONY: default
default: $(EIF_NAME)

# Build the container image ------------------------------------
.PHONY: image
image:
	mkdir -p out
	docker buildx build --platform linux/arm64 -t $(IMAGE_NAME) -f Containerfile .
# Create the Enclave Image File (EIF) ---------------------------
$(EIF_NAME): image
	mkdir -p $(OUT_DIR)
	nitro-cli build-enclave \
		--docker-uri $(IMAGE_NAME) \
		--output-file $(EIF_NAME)

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

.PHONY: update
update:
	cd ../../../.. && git pull origin main && cd packages/tee-login/dev/tee-arm && rm -rf out
	$(MAKE) image
	$(MAKE) eif