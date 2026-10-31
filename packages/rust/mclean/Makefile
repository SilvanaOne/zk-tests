# mclean: previews, confirms and removes node_modules, .next and target build output and every file older than 30
# days in logs/devnet, logs/testnet and logs/mainnet below the current directory, on macOS thins local Time Machine
# snapshots, and prunes Docker.
# Style: ai-env/Makefile (`make help` lists targets from `## comments`); GNU make 3.81 (macOS /usr/bin/make) and 4.x.
.PHONY: help build test fmt clippy check install

SHELL        := /bin/bash
# The channel of rust-toolchain.toml; `make TOOLCHAIN=x` overrides it.
TOOLCHAIN    := $(shell sed -n 's/^channel = "\([^"]*\)".*/\1/p' rust-toolchain.toml)
CARGO        := cargo +$(TOOLCHAIN)
# A make started under `cargo run` inherits CARGO (the path cargo gives its children), and make would export this
# file's value to every recipe; recipes use $(CARGO), their children find cargo on PATH.
unexport CARGO
# Where `make install` puts mclean (cargo install --root): CARGO_INSTALL_ROOT, CARGO_HOME or ~/.cargo, as cargo
# does, except that cargo's install.root config is not read. Empty means the default; a trailing slash is dropped.
override INSTALL_ROOT := $(patsubst %/,%,$(or $(INSTALL_ROOT),$(CARGO_INSTALL_ROOT),$(CARGO_HOME),$(HOME)/.cargo))
INSTALLED    := $(INSTALL_ROOT)/bin/mclean
# The [package] version, which the installed binary must report.
PKG_VERSION  := $(shell sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)

help: ## Show this help
	@grep -h -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'

build: ## Debug build (target/debug/mclean)
	$(CARGO) build

test: ## Unit and CLI tests (scratch trees under the system temp dir; fake docker, tmutil and df only)
	$(CARGO) test

fmt: ## rustfmt check (`cargo fmt` applies it)
	$(CARGO) fmt -- --check

clippy: ## clippy -D warnings, tests included
	$(CARGO) clippy --all-targets -- -D warnings

check: fmt clippy test ## fmt + clippy + test

# mclean builds before 2.0.0 ignore their arguments and delete at once, --version included: the version check proves
# the new binary is the one installed, and every other mclean on PATH is named so it can be deleted by hand.
install: ## cargo install mclean into $(INSTALL_ROOT)/bin with the committed Cargo.lock; checks its --version, warns about other copies on PATH
	$(CARGO) install --path . --locked --root "$(INSTALL_ROOT)"
	@v=$$("$(INSTALLED)" --version) || exit 1; echo "installed: $$v ($(INSTALLED))"; \
	  test "$$v" = "mclean $(PKG_VERSION)" || { echo "install: expected mclean $(PKG_VERSION), the binary printed: $$v"; exit 1; }
	@case ":$$PATH:" in *":$(INSTALL_ROOT)/bin:"*) ;; *) echo "note: $(INSTALL_ROOT)/bin is not on PATH";; esac
	@which -a mclean 2>/dev/null | awk '!seen[$$0]++' | while IFS= read -r p; do \
	  [ "$$p" -ef "$(INSTALLED)" ] || echo "warning: another mclean on PATH: $$p (delete it by hand: an old build ignores --dry-run)"; \
	done
