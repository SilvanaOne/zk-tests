# Silvana TEE login

This TEE configuration is based on Nautilus TEE configuration
https://github.com/MystenLabs/nautilus

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
ssh -i "TEE.pem" ec2-user@ec2-23-21-249-129.compute-1.amazonaws.com
```

and in terminal run

```sh
git clone https://github.com/SilvanaOne/zk-tests
cd zk-tests/packages/tee-login/tee/x86
make
```

on update

```sh
cd ../../.. && git pull origin main && cd packages/tee-login/tee/x86 && rm -rf out && make
```

after make

```sh
make run-debug
```

or, in production

```sh
make run
```

and in other terminal

```sh
sh expose_enclave.sh
```

To stop enclave:

```sh
sh reset_enclave.sh
```

copy image to s3 and back:

```sh
tar -czvf out.tar.gz out
aws s3 cp out.tar.gz s3://silvana-tee-images/tee.tar.gz
aws s3 cp s3://silvana-tee-images/tee.tar.gz tee.tar.gz
tar -xzvf tee.tar.gz
```

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/stats

curl -H 'Content-Type: application/json' -X GET http://23.21.249.129:3000/get_attestation

curl -H 'Content-Type: application/json' -d '{"payload": { "memo": "agent"}}' -X POST http://54.242.34.226:3000/login

curl -H 'Content-Type: application/json' -d '{"payload": { "memo": "hi"}}' -X POST http://23.21.249.129:3000/ping

sudo dnf update -y
sudo dnf install -y nginx certbot python3-certbot-nginx
sudo certbot --nginx -d tee.silvana.dev

/etc/nginx/conf.d/tee.silvana.dev.conf

```
server {
    listen 443 ssl;
    server_name tee.silvana.dev;

    ssl_certificate     /etc/letsencrypt/live/tee.silvana.dev/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/tee.silvana.dev/privkey.pem;

    location / {
        proxy_pass         http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
    }
}

# Optionally also redirect HTTP → HTTPS:
server {
    listen      80;
    server_name tee.silvana.dev;
    return      301 https://$host$request_uri;
}

```

sudo systemctl enable --now nginx
sudo nginx -t # test syntax
sudo systemctl reload nginx
