build:
	cargo build

lambda:
	cargo lambda build --release --bin bootstrap -p lambda

deploy:
	cd pulumi && pulumi up

preview:
	cd pulumi && pulumi preview

test:
	@echo "Testing Lambda function..."
	@curl -X POST https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws/ \
		-H "Content-Type: application/json" \
		-d '{"command": "test-command"}' \
		| python3 -m json.tool

test-api:
	@echo "Testing Lambda via API Gateway..."
	@curl -X POST https://wprchl5wjd.execute-api.us-east-1.amazonaws.com/stage/ \
		-H "Content-Type: application/json" \
		-d '{"command": "test-via-api"}' \
		| python3 -m json.tool