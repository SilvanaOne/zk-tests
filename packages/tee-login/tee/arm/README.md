cd ../../../.. && git pull origin main && cd packages/tee-login/tee/arm && rm -rf out && make

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/health_check

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/stats

curl -H 'Content-Type: application/json' -X GET http://35.174.157.43:3000/get_attestation
