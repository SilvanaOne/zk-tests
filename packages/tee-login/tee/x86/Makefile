REGISTRY := local
.DEFAULT_GOAL :=
.PHONY: default
default: out/enclaveos.tar

out:
	mkdir out

out/enclaveos.tar: out \
	$(shell git ls-files \
		src/init \
		src/aws \
        src/hello \
	)
	docker build \
		--tag $(REGISTRY)/enclaveos \
		--progress=plain \
		--platform linux/amd64 \
		--output type=local,rewrite-timestamp=true,dest=out\
		-f Containerfile \
		.
	@echo "Pruning Docker artifacts..."
	docker image prune -f
	docker container prune -f

.PHONY: run
run: out/nitro.eif
	sudo nitro-cli \
		run-enclave \
		--cpu-count 8 \
		--memory 16384M \
		--eif-path out/nitro.eif

.PHONY: run-tiny
run-tiny: out/nitro.eif
	sudo nitro-cli \
		run-enclave \
		--cpu-count 2 \
		--memory 4096M \
		--eif-path out/nitro.eif

.PHONY: run-debug
run-debug: out/nitro.eif
	sudo nitro-cli \
		run-enclave \
		--cpu-count 8 \
		--memory 16384M \
		--eif-path out/nitro.eif \
		--debug-mode \
		--attach-console

.PHONY: run-tiny-debug
run-tiny-debug: out/nitro.eif
	sudo nitro-cli \
		run-enclave \
		--cpu-count 2 \
		--memory 4096M \
		--eif-path out/nitro.eif \
		--debug-mode \
		--attach-console

.PHONY: update
update:
	./update.sh

