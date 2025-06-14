# Silvana TEE login

This TEE configuration is based on Nautilus TEE configuration

## Create AWS keys

### AWS EC2

- AWS EC2 ED25519 KeyPair called TEE

### AWS KMS

AWS KMS KeyPair with the following configuration:

- Key Type: Symmetric
- Key Spec: SYMMETRIC_DEFAULT
- Key Usage: Encrypt and Decript
- Key material origin - KMS
- Name: TEEKMS
- User: silvana-tee-api-user TODO: refactor to create a user before KMS key

## Install AWS Stack with pulumi

```sh
brew install pulumi/tap/pulumi
pulumi version
pulumi login --local
cd pulumi
npm i
export AWS_ACCESS_KEY_ID=<YOUR_KEY_ID>
export AWS_SECRET_ACCESS_KEY=<YOUR_ACCESS_KEY>
export AWS_REGION=us-east-1
export PULUMI_CONFIG_PASSPHRASE=<CREATE_YOUR_PULUMI_PASSWORD>
pulumi up
pulumi stack output
```

Check deployment:

```sh
pulumi stack output --show-secrets
```

```
Current stack outputs (4):
    OUTPUT          VALUE
    apiAccessKeyId  ...
    apiSecretKey    ...
    bucketName      silvana-tee-login-b29fc12
    tableName       silvana-tee-login-7d0f6df
```

To start from scratch:

```sh
pulumi destroy
pulumi stack rm
```

## Connect with AWS instance

```sh
ssh -i "TEE.pem" ec2-user@ec2-3-85-252-150.compute-1.amazonaws.com
```
