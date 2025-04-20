RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor build
RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor test --skip-local-validator
export RUSTFLAGS="--cfg procmacro2_semver_exempt"   

solana-test-validator

CARGO_NET_GIT_FETCH_WITH_CLI=true  RUSTFLAGS='--cfg procmacro2_semver_exempt' anchor build 
objdump -h target/deploy/f.so

BN254:
Poseidon transaction: 478.437ms