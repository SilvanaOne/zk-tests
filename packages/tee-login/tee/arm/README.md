cd ../../../.. && git pull origin main && cd packages/tee-login/tee/arm && rm -rf out && make

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://54.82.214.64:3000/stats

curl -H 'Content-Type: application/json' -X GET http://35.175.45.79:3000/get_attestation

run:
ssh -i "TEE.pem" ec2-user@35.175.45.79
dev:
ssh -i "TEE.pem" ec2-user@54.82.214.64
sudo less /var/log/cloud-init-output.log

```sh
git clone https://github.com/SilvanaOne/zk-tests
cd zk-tests/packages/tee-login/tee/arm
make
```

copy image to s3 and back:

```sh
tar -czvf tee-arm-v5.tar.gz out
aws s3 cp tee-arm-v5.tar.gz s3://silvana-tee-images/tee-arm-v5.tar.gz
aws s3 cp time-server-v1.tar.gz s3://silvana-tee-images/time-server-v1.tar.gz
aws s3 cp s3://silvana-tee-images/tee-arm-v5.tar.gz tee-arm-v5.tar.gz
tar -xzvf tee-arm-v5.tar.gz
aws s3 cp s3://silvana-tee-images/time-server-v1.tar.gz time-server-v1.tar.gz
tar -xzvf time-server-v1.tar.gz
```

Job for nitro-enclaves-allocator.service failed because the control process exited with error code.
See "systemctl status nitro-enclaves-allocator.service" and "journalctl -xeu nitro-enclaves-allocator.service" for details.

"Token has expired: claim.iat:1750893282 claim.exp:1750896882 now:1750917839"