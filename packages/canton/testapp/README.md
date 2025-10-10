## Localnet installation

https://docs.dev.sync.global/validator_operator/validator_compose.html
https://docs.dev.sync.global/app_dev/testing/localnet.html
https://github.com/digital-asset/decentralized-canton-sync/releases/download/v0.4.19/0.4.19_splice-node.tar.gz
https://docs.sync.global/app_dev/scan_api/scan_openapi.html

```sh
tar xzvf 0.4.19_splice-node.tar.gz
export JAVA_HOME=$(/usr/libexec/java_home -v 21) && export PATH="$JAVA_HOME/bin:$PATH" && echo "Using JAVA_HOME=$JAVA_HOME"
just start
just admin
@ `app-provider`.adminToken
res0: Option[String] = Some(
value = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJhdWQiOiJodHRwczovL2NhbnRvbi5uZXR3b3JrLmdsb2JhbCIsInN1YiI6ImxlZGdlci1hcGktdXNlciJ9.A0VZW69lWWNVsjZmDDpVvr1iQ_dJLga3f-K2bicdtsc"
)

@ `app-user`.adminToken
res1: Option[String] = Some(
value = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJhdWQiOiJodHRwczovL2NhbnRvbi5uZXR3b3JrLmdsb2JhbCIsInN1YiI6ImxlZGdlci1hcGktdXNlciJ9.A0VZW69lWWNVsjZmDDpVvr1iQ_dJLga3f-K2bicdtsc"
)
```

## Building and Deploying the SimpleSumContract

### Prerequisites

- Canton LocalNet running (see installation above)
- DAML SDK installed
- Make sure the JWT tokens in `.env` match your Canton instance

### Step 1: Build the DAML Package

Build the DAML project to create the DAR file:

```bash
# From the testapp directory
daml build

# This creates .daml/dist/testapp-0.0.1.dar
```

### Step 2: Upload the DAR Package

Upload the DAR file to the app-user participant:

```bash
# Upload to app-user (default - use this for creating contracts as app-user)
make upload DAR_FILE=.daml/dist/testapp-0.0.1.dar

# Alternatively, upload to app-provider (if you want app-provider to create contracts)
make upload-provider DAR_FILE=.daml/dist/testapp-0.0.1.dar
```

```
Uploading DAR file .daml/dist/testapp-0.0.1.dar to app-user...
{}
```

### Step 3: Create the Contract

Create a SimpleSumContract with app-user as the prover:

```bash
make create
```

This creates a contract with:

- **prover**: app_user_localnet-localparty-1 (the owner)
- **sum**: 0 (initial value)
- **observers**: [] (empty list initially)

The response will show the created contract details including the contract ID. Note this contract ID - you'll need it for subsequent operations.

```
testapp % make create
Creating SimpleSumContract with app-user as prover...
{
  "transaction": {
    "updateId": "12207e03fc7d508650ce4202b1739f9715977b1bc4e11aab9a5b37ebeacdcab83926",
    "commandId": "create-1759765230",
    "workflowId": "",
    "effectiveAt": "2025-10-06T15:40:30.727143Z",
    "events": [
      {
        "CreatedEvent": {
          "offset": 83,
          "nodeId": 0,
          "contractId": "006710c328fc39a770ada03c3cc7bcd7ddca315555464923dac33c4925b832123aca111220852a648d5f5971e619982ac719b7f3139d16a1e9f8b0901aa0895ab3206f5b9d",
          "templateId": "889528a2d89a92f68c42a267abe105fbf770f8c772661f68ac87b55429e8cbe5:Main:SimpleSumContract",
          "contractKey": null,
          "createArgument": {
            "prover": "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710",
            "sum": "0",
            "observers": []
          },
          "createdEventBlob": "",
          "interfaceViews": [],
          "witnessParties": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "signatories": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "observers": [],
          "createdAt": "2025-10-06T15:40:30.727143Z",
          "packageName": "testapp"
        }
      }
    ],
    "offset": 83,
    "synchronizerId": "global-domain::1220fb2f7c5c34f176231083a00dd6fd953eead685261e5d792d9bf94df44e88463a",
    "traceContext": {
      "traceparent": "00-3fec05eabb7041f39ac0f57ee374bd26-cfc63999cf6092b0-01",
      "tracestate": null
    },
    "recordTime": "2025-10-06T15:40:30.916468Z"
  }
}
```

### Step 4: Add Values to the Contract

Use the AddValue choice to add values to the sum:

```bash
# Add 50 to the sum
make add CONTRACT_ID=<your-contract-id> VALUE=50

# Add another 25 to the sum
make add CONTRACT_ID=<your-contract-id> VALUE=25
```

Each call creates a new version of the contract with the updated sum. The response shows the new contract ID and the events generated.

```
testapp % make add CONTRACT_ID=006710c328fc39a770ada03c3cc7bcd7ddca315555464923dac33c4925b832123aca111220852a648d5f5971e619982ac719b7f3139d16a1e9f8b0901aa0895ab3206f5b9d VALUE=100
Adding value 100 to contract 006710c328fc39a770ada03c3cc7bcd7ddca315555464923dac33c4925b832123aca111220852a648d5f5971e619982ac719b7f3139d16a1e9f8b0901aa0895ab3206f5b9d...
{
  "transaction": {
    "updateId": "1220eb6553596a4706c3fe101389f56d5c36dfdbc1a518a6cd97a3d29fa6eeeacbf4",
    "commandId": "add-value-1759765408",
    "workflowId": "",
    "effectiveAt": "2025-10-06T15:43:28.129666Z",
    "events": [
      {
        "ArchivedEvent": {
          "offset": 95,
          "nodeId": 0,
          "contractId": "006710c328fc39a770ada03c3cc7bcd7ddca315555464923dac33c4925b832123aca111220852a648d5f5971e619982ac719b7f3139d16a1e9f8b0901aa0895ab3206f5b9d",
          "templateId": "889528a2d89a92f68c42a267abe105fbf770f8c772661f68ac87b55429e8cbe5:Main:SimpleSumContract",
          "witnessParties": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "packageName": "testapp",
          "implementedInterfaces": []
        }
      },
      {
        "CreatedEvent": {
          "offset": 95,
          "nodeId": 1,
          "contractId": "00e09438c4e707497b836b6a01857fdf187721c27c8fcdd0e8cfec9a3d0bbdae61ca111220af0e66e7b9ebd528655fdfe75584f1d8472c3a618e72829acc04f01790325301",
          "templateId": "889528a2d89a92f68c42a267abe105fbf770f8c772661f68ac87b55429e8cbe5:Main:SimpleSumContract",
          "contractKey": null,
          "createArgument": {
            "prover": "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710",
            "sum": "100",
            "observers": []
          },
          "createdEventBlob": "",
          "interfaceViews": [],
          "witnessParties": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "signatories": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "observers": [],
          "createdAt": "2025-10-06T15:43:28.129666Z",
          "packageName": "testapp"
        }
      }
    ],
    "offset": 95,
    "synchronizerId": "global-domain::1220fb2f7c5c34f176231083a00dd6fd953eead685261e5d792d9bf94df44e88463a",
    "traceContext": {
      "traceparent": "00-a07a0d80b13309e81ac37db574dcbc32-1e93e0b6ee400b20-01",
      "tracestate": null
    },
    "recordTime": "2025-10-06T15:43:28.157059Z"
  }
}
```

### Step 6: Add Observer to the Contract

Add app-provider as an observer to allow them to view the contract:

```bash
make observer CONTRACT_ID=<your-contract-id>
```

This adds the app-provider party as an observer, creating a new version of the contract. Once added as an observer, app-provider can:

- View the contract state
- Query the sum using the GetSum choice
- See updates related to the contract

```
testapp % make observer CONTRACT_ID=00e09438c4e707497b836b6a01857fdf187721c27c8fcdd0e8cfec9a3d0bbdae61ca111220af0e66e7b9ebd528655fdfe75584f1d8472c3a618e72829acc04f01790325301
Adding app-provider as observer to contract 00e09438c4e707497b836b6a01857fdf187721c27c8fcdd0e8cfec9a3d0bbdae61ca111220af0e66e7b9ebd528655fdfe75584f1d8472c3a618e72829acc04f01790325301...
{
  "transaction": {
    "updateId": "1220a2cc65b010439919b768261e5e291dd97311c08f28c2706019d12295c8561c7f",
    "commandId": "add-observer-1759765943",
    "workflowId": "",
    "effectiveAt": "2025-10-06T15:52:23.565063Z",
    "events": [
      {
        "ArchivedEvent": {
          "offset": 132,
          "nodeId": 0,
          "contractId": "00e09438c4e707497b836b6a01857fdf187721c27c8fcdd0e8cfec9a3d0bbdae61ca111220af0e66e7b9ebd528655fdfe75584f1d8472c3a618e72829acc04f01790325301",
          "templateId": "889528a2d89a92f68c42a267abe105fbf770f8c772661f68ac87b55429e8cbe5:Main:SimpleSumContract",
          "witnessParties": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "packageName": "testapp",
          "implementedInterfaces": []
        }
      },
      {
        "CreatedEvent": {
          "offset": 132,
          "nodeId": 1,
          "contractId": "00ee44f332e37e5c8163295288dcdb7cb0eae49a9dc046ef091ac458497c2d35f1ca111220665dd0837fae82629a37b252fc3367f26cc18f5f950c00d629d3ff6014d64ae6",
          "templateId": "889528a2d89a92f68c42a267abe105fbf770f8c772661f68ac87b55429e8cbe5:Main:SimpleSumContract",
          "contractKey": null,
          "createArgument": {
            "prover": "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710",
            "sum": "100",
            "observers": [
              "app_provider_localnet-localparty-1::12203c933671e4c0f69dc4c136e374c47689c2b65d70e4e64c7f3897c636353ae4fd"
            ]
          },
          "createdEventBlob": "",
          "interfaceViews": [],
          "witnessParties": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "signatories": [
            "app_user_localnet-localparty-1::1220cb222d9c97c8f3dba110cb745698957a3488b456a81067cbd68304e4c7118710"
          ],
          "observers": [
            "app_provider_localnet-localparty-1::12203c933671e4c0f69dc4c136e374c47689c2b65d70e4e64c7f3897c636353ae4fd"
          ],
          "createdAt": "2025-10-06T15:52:23.565063Z",
          "packageName": "testapp"
        }
      }
    ],
    "offset": 132,
    "synchronizerId": "global-domain::1220fb2f7c5c34f176231083a00dd6fd953eead685261e5d792d9bf94df44e88463a",
    "traceContext": {
      "traceparent": "00-6b2ec42b6001f4ec348b4e0e5abf197e-8b01a6d010618d1b-01",
      "tracestate": null
    },
    "recordTime": "2025-10-06T15:52:23.606546Z"
  }
}
```

### Step 7: View Ledger Updates

You can query the ledger to see recent updates and transactions:

```bash
# Get the last 100 updates (default)
make updates

# Get updates from a specific offset
make updates OFFSET=50

# Get all updates from the beginning
make updates OFFSET=0
```

The updates show all transactions, contract creations, and exercises visible to app-user, including:

- Contract creation events
- Exercise events (AddValue, AddObserver, etc.)
- Offset checkpoints
- Transaction metadata

## Available Commands

### Party Management

- `make parties-user` - List all parties visible to app-user
- `make parties-provider` - List all parties visible to app-provider

### Contract Deployment

- `make upload DAR_FILE=<path>` - Upload a DAR file to app-user
- `make upload-provider DAR_FILE=<path>` - Upload a DAR file to app-provider
- `make create` - Create a SimpleSumContract with app-user as prover

### Contract Operations

- `make add CONTRACT_ID=<id> VALUE=<int>` - Add a value to the sum (prover only)
- `make observer CONTRACT_ID=<id>` - Add app-provider as observer to the contract

### Ledger Queries

- `make updates` - Get last 100 ledger updates for app-user
- `make updates OFFSET=<n>` - Get ledger updates from specific offset

## Contract Template

The SimpleSumContract template includes:

- **AddValue** choice: Add a value to the sum (prover only)
- **AddObserver** choice: Add a party as an observer (prover only)
- **RemoveObserver** choice: Remove an observer (prover only)
- **GetSum** choice: Query the current sum (prover or observers)
