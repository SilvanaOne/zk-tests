
export RUSTFLAGS="--cfg procmacro2_semver_exempt"   
export CARGO_BUILD_RUSTFLAGS="--cfg procmacro2_semver_exempt"   
export CARGO_ENCODED_RUSTFLAGS="--cfg procmacro2_semver_exempt"  
RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor build
RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor test --skip-local-validator

solana-test-validator --compute-unit-limit 1000000000

CARGO_NET_GIT_FETCH_WITH_CLI=true  RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor build 
objdump -h target/deploy/poseidon.so

BN254:
Poseidon transaction: 478.437ms

cargo update -p proc-macro2 --precise 1.0.94
RUSTUP_TOOLCHAIN=nightly-2025-04-14 anchor build
RUSTUP_TOOLCHAIN=nightly-2025-04-14 anchor deploy
RUSTUP_TOOLCHAIN=nightly-2025-04-14 anchor test --skip-local-validator