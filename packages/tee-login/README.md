# Silvana TEE login

## Structure

### tee

#### x86

- TEE deployment code for x86 (used in https://login.silvana.dev/)
- main rust repo: src/server
- use several crypto libraries, including proof-systems
- connected with KMS for key management
- connected with DynamoDB for encrypted state storage

#### arm

- TEE deployment code for arm64 (WIP)

#### pulumi

- AWS deployment script

### ui

- Main UI https://login.silvana.de

### ui-api

- TypeScript isolated enclave without internet access that keep encrypted private keys in runtime in browser
- Use rust code compiled to wasm and mina-signer

### ui-precompiles

- rust code for compilation to wasm. Use proof-systems repo and other crypto libraries. Share some code with main TEE server repo

### dev

Repo for development tests
