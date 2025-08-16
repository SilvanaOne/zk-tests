# Lambda example

## Login to pulimi

```sh
pulumi login --local
```

## Configure AWS

```sh
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1
export PULUMI_CONFIG_PASSPHRASE=...
```

## Build lambda

```sh
make lambda
```

## Deploy lambda

```sh
make deploy
```
