cd ../../../.. && git pull origin main && cd packages/tee-login/tee/arm && rm -rf out && make

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/stats

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/get_attestation

copy image to s3 and back:

```sh
tar -czvf tee-arm.tar.gz out
aws s3 cp tee-arm.tar.gz s3://silvana-tee-images/tee-arm.tar.gz
aws s3 cp s3://silvana-tee-images/tee-arm.tar.gz tee-arm.tar.gz
tar -xzvf tee-arm.tar.gz
```
