# Canton Billing System - Deployment Guide for x86 Canton Node

This guide provides step-by-step instructions for deploying the Canton Billing System on an x86 Canton node.

## Prerequisites

- Canton node running and accessible
- Canton Scan API accessible
- App Provider API accessible with JWT authentication

## Building the x86 Binary

### 1. Build the Binary

From the billing directory, build the x86_64 Linux binary using Docker cloud builder (change docker cloud builder name as necessary):

```bash
make build-x86
```

This will:

- Use the Docker cloud builder to compile for x86_64 Linux
- Create the binary at `target/x86_64-unknown-linux-gnu/release/billing`

### 2. Prepare Deployment Package

Copy the binary and configuration files to the `x86/` directory:

```bash
make copy
```

This creates the `x86/` directory with:

- `billing` - The compiled binary
- `.env` - Environment configuration
- `users.json` - User definitions
- `subscriptions.json` - Subscription plans

## Deployment Steps

### Step 1: Configure Environment Variables

Edit the `.env` file in the `x86/` directory with your Canton node details:

**Required Configuration:**

- `SCAN_API_URL` - Canton Scan API endpoint
- `APP_PROVIDER_API_URL` - Canton Provider API endpoint
- `APP_PROVIDER_JWT` - JWT token for API authentication

### Step 2: Create and Configure App Party

#### 2.1 Create App Party

Create a new party on the Canton network for the billing application. This can be done through your Canton node's admin interface or CLI.

Once created, add the party ID to `.env`:

```bash
PARTY_APP=app1::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10
```

#### 2.2 Onboard the Party

Onboard the party to the Canton network using your Canton node's onboarding process.

#### 2.3 Make it a Featured App

Configure the party as a featured app in your Canton network configuration.

### Step 3: Fund the App Party

Transfer a small amount of Canton Credits (CC) to the app party for transaction fees.

**Recommended initial amount:** 100 CC

#### 3.1 Accept the Transfer

Once the transfer is initiated, accept it using your Canton tools or the billing CLI.

### Step 4: Verify Balance

Check that the app party has received the Canton Credits:

```bash
./billing balance
```

Expected output:

```
Getting balance for party app1::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10...

Found Amulet contracts:
----------------------------------------
Contract ID: 00f10ca7a065d4e251427791de495ba20388ac658868cb568487ddd83446a82f81ca11122021d001ac22e329e8080e63603bae05e845f67b7564a912f24e333a0222282216
Amount:      100 CC
Round:       254

----------------------------------------
Summary:
  Total Amulets: 1
  Total Balance: 100 CC
```

### Step 5: Setup TransferPreapproval

Initialize the TransferPreapproval contract that enables automated recurring payments:

```bash
./billing setup
```

This will create a TransferPreapproval contract valid for 1 year (default) and display output like:

```
✅ TransferPreapproval created successfully!

📋 TransferPreapproval Contract ID:
   00813bd58e439dda09ff0c9cf8539cc4de5a4e0f7b12a497ffde8a1e9167d49af8ca11122075b85cdac603f6d2cc3dedb1d4329d1af1247b22c9b656b31a08d30fe632dc29

Add to .env:
   APP_TRANSFER_PREAPPROVAL_CID=00813bd58e439dda09ff0c9cf8539cc4de5a4e0f7b12a497ffde8a1e9167d49af8ca11122075b85cdac603f6d2cc3dedb1d4329d1af1247b22c9b656b31a08d30fe632dc29
```

Add the TransferPreapproval contract ID to your `.env` file:

```bash
APP_TRANSFER_PREAPPROVAL_CID=00813bd58e439dda09ff0c9cf8539cc4de5a4e0f7b12a497ffde8a1e9167d49af8ca11122075b85cdac603f6d2cc3dedb1d4329d1af1247b22c9b656b31a08d30fe632dc29
```

### Step 6: Check configuration

run

```sh
./billing config
```

It should give the result like

```
Fetching contract blobs context from Canton network...

📋 Contract Blobs Context
═══════════════════════════════════════════════════════════════

🔐 Synchronizer ID:
   global-domain::122075d227a0482dc186fa09a3ddc4e0b2046d1ce9fdf4ec7375bd698b362525632e

💰 AmuletRules:
   Contract ID:  007fccae622387b865cc0aa564b5a9956c177d0dd678238585c9f10dd8e5fd6c8bca1112205b62b6502b4ead7f177c0937740190d137e15686fd49c308808d0086b43276cf
   Template ID:  3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.AmuletRules:AmuletRules
   Blob (first 80 chars): CgMyLjESng8KRQB/zK5iI4e4ZcwKpWS1qZVsF30N1ngjhYXJ8Q3Y5f1si8oREiBbYrZQK06tfxd8CTd0...

⛏️  OpenMiningRound:
   Contract ID:  002f0ab711e0d5bf6ee9258a63e2c134370e84778d9f1844652ee32397400b01aeca1112201ac1fe979e39a014f4b41a754a9e9aea860ad1eca14ac1c7f9eadaf56a43a092
   Template ID:  3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound
   Blob (first 80 chars): CgMyLjESngcKRQAvCrcR4NW/buklimPiwTQ3DoR3jZ8YRGUu4yOXQAsBrsoREiAawf6XnjmgFPS0GnVK...

⭐ FeaturedAppRight:
   Contract ID:  00ffeec80be6d24e0a94dbd66fb51f4d01233f878b33cdc0e1f17aad56426c604aca111220ac6762377fc71c9744ad041bb995cd7c8e2ff2b55891dfe3600e345824632f3c
   Template ID:  3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Amulet:FeaturedAppRight
   Blob (first 80 chars): CgMyLjESrQQKRQD/7sgL5tJOCpTb1m+1H00BIz+HizPNwOHxeq1WQmxgSsoREiCsZ2I3f8ccl0StBBu5...

✅ Context fetched successfully!
```

### Step 7: Start the Billing Service

Add the subscriptions to subscriptions.json and users to users.json an then start the automated payment processing:

```bash
./billing start
```

The service will:

- Process payments every 60 seconds (default interval)
- Automatically charge users based on their subscription billing intervals
- Track and export metrics to OpenTelemetry (if configured)
- Retry failed payments with exponential backoff

the output will be similar to

```
2025-10-11T13:21:16.463081Z  INFO billing::cli: Payment due user=User 2 subscription=verifier amount=1 CC description=verifier subscription payment for User 2 interval_secs=600 dry_run=false
2025-10-11T13:21:16.510235Z  INFO billing::pay: Found Amulet contract cid=0082fa37760e366fd4882fcc43e9758462e55891a480809f57d758b497a052f79eca111220b55742f86271c249d8d786361b396d27163d371faa693356c95b3a1050578dc8 amount=119923.0000000000
2025-10-11T13:21:16.510318Z  INFO billing::pay: Executing TransferPreapproval_Send from=userparty2::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10 to=app1::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10 amount=1 description=verifier subscription payment for User 2
2025-10-11T13:21:16.898867Z  INFO billing::pay: Payment successful amount=1 from=userparty2::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10 to=app1::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10 update_id=1220d632fa38886d06800269d6cc42713b26b57d66e4352c0b04a1297744a6ac4401
2025-10-11T13:21:16.899028Z  INFO billing::metrics: Payment recorded successfully user=userparty2::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10 subscription=verifier amount=1 command_id=pay-verifier-userparty2-1760188876
2025-10-11T13:21:16.899048Z  INFO billing::cli: Payment executed successfully user=User 2 subscription=verifier amount=1.0 command_id=pay-verifier-userparty2-1760188876 update_id=1220d632fa38886d06800269d6cc42713b26b57d66e4352c0b04a1297744a6ac4401
2025-10-11T13:21:16.899161Z  INFO billing::cli: Payment successful user=User 2 subscription=verifier amount=1 CC description=verifier subscription payment for User 2

```

**Options:**

```bash
# Run once only (useful for testing)
./billing start --once

# Custom check interval (300 seconds)
./billing start --interval 300

# Dry run mode (simulate without executing)
./billing start --dry-run
```

## Checking updates

You can check any update from log by running

```sh
./billing update 1220a28e2b2696faf59c0ee7057b0ec07267fca05377ac88e0c84756b515fa634c10
```
