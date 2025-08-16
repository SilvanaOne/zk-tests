import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";
import * as path from "path";
import * as fs from "fs";

export = async () => {
  // Get current AWS region
  const region = await aws.getRegion();
  const accountId = await aws.getCallerIdentity();

  // Get Pulumi config for Sui secrets
  const config = new pulumi.Config();
  const suiPackageId = config.getSecret("suiPackageId");
  const suiChain = config.getSecret("suiChain");
  const suiAddress = config.getSecret("suiAddress");
  const suiSecretKey = config.getSecret("suiSecretKey");

  // Create an S3 bucket for Lambda deployment packages
  const deploymentBucket = new aws.s3.Bucket("lambda-deployment-bucket", {
    bucket: "silvana-lambda",
    forceDestroy: true, // Allow bucket to be deleted even if it contains objects
    tags: {
      Name: "silvana-lambda",
      Purpose: "Lambda function deployments",
    },
  });

  // Enable versioning on the bucket
  new aws.s3.BucketVersioning("lambda-deployment-bucket-versioning", {
    bucket: deploymentBucket.id,
    versioningConfiguration: {
      status: "Enabled",
    },
  });

  // Create IAM role for Lambda execution
  const lambdaRole = new aws.iam.Role("lambda-execution-role", {
    assumeRolePolicy: JSON.stringify({
      Version: "2012-10-17",
      Statement: [
        {
          Effect: "Allow",
          Principal: {
            Service: "lambda.amazonaws.com",
          },
          Action: "sts:AssumeRole",
        },
      ],
    }),
    tags: {
      Name: "lambda-execution-role",
    },
  });

  // Attach basic Lambda execution policy
  const lambdaBasicExecutionPolicy = new aws.iam.RolePolicyAttachment(
    "lambda-basic-execution",
    {
      role: lambdaRole.name,
      policyArn:
        "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
    }
  );

  // Create KMS key for encrypting private keys
  const kmsKey = new aws.kms.Key("sui-keypair-encryption-key", {
    description: "KMS key for encrypting Sui private keys",
    keyUsage: "ENCRYPT_DECRYPT",
    customerMasterKeySpec: "SYMMETRIC_DEFAULT",
    tags: {
      Name: "sui-keypair-encryption",
      Purpose: "Encrypt Sui private keys at rest",
    },
  });

  // Create KMS key alias for easier reference
  const kmsKeyAlias = new aws.kms.Alias("sui-keypair-encryption-alias", {
    name: "alias/sui-keypair-encryption",
    targetKeyId: kmsKey.id,
  });

  // Create custom policy for additional permissions
  const lambdaCustomPolicy = new aws.iam.RolePolicy("lambda-custom-policy", {
    role: lambdaRole.id,
    policy: pulumi.interpolate`{
      "Version": "2012-10-17",
      "Statement": [
        {
          "Effect": "Allow",
          "Action": [
            "logs:CreateLogGroup",
            "logs:CreateLogStream",
            "logs:PutLogEvents"
          ],
          "Resource": "arn:aws:logs:*:*:*"
        },
        {
          "Effect": "Allow",
          "Action": [
            "s3:GetObject",
            "s3:PutObject"
          ],
          "Resource": "${deploymentBucket.arn}/*"
        },
        {
          "Effect": "Allow",
          "Action": [
            "dynamodb:GetItem",
            "dynamodb:PutItem",
            "dynamodb:DeleteItem",
            "dynamodb:UpdateItem"
          ],
          "Resource": [
            "arn:aws:dynamodb:us-east-1:${accountId.accountId}:table/sui-key-locks",
            "arn:aws:dynamodb:us-east-1:${accountId.accountId}:table/sui-keypairs"
          ]
        },
        {
          "Effect": "Allow",
          "Action": [
            "kms:Decrypt",
            "kms:GenerateDataKey"
          ],
          "Resource": "${kmsKey.arn}"
        }
      ]
    }`,
  });

  // Create DynamoDB table for storing encrypted keypairs
  const keypairsTable = new aws.dynamodb.Table("sui-keypairs", {
    name: "sui-keypairs",
    billingMode: "PAY_PER_REQUEST", // On-demand pricing
    hashKey: "id", // Binary composite key (login_type + login)
    attributes: [
      {
        name: "id",
        type: "B", // Binary
      },
    ],
    tags: {
      Name: "sui-keypairs",
      Purpose: "Store encrypted Sui keypairs",
    },
  });

  // Create DynamoDB table for Sui key locks
  const locksTable = new aws.dynamodb.Table("sui-key-locks", {
    name: "sui-key-locks",
    billingMode: "PAY_PER_REQUEST", // On-demand pricing
    hashKey: "address",
    rangeKey: "chain",
    attributes: [
      {
        name: "address",
        type: "S", // String
      },
      {
        name: "chain",
        type: "S", // String (devnet, testnet, mainnet)
      },
    ],
    ttl: {
      attributeName: "expires_at",
      enabled: true, // Automatically delete expired locks
    },
    tags: {
      Name: "sui-key-locks",
      Purpose: "Prevent concurrent Sui transactions",
    },
  });

  // Create CloudWatch Log Group for Lambda
  const logGroup = new aws.cloudwatch.LogGroup("lambda-log-group", {
    name: "/aws/lambda/rust-lambda-function",
    retentionInDays: 7,
    tags: {
      Name: "lambda-logs",
    },
  });

  // Path to the Lambda binary directory (built with cargo lambda)
  // The bootstrap binary is at target/lambda/bootstrap/bootstrap
  const lambdaBinaryPath = path.join(
    __dirname,
    "..",
    "target",
    "lambda",
    "bootstrap",
    "bootstrap"
  );

  // Check if the binary exists, if not provide a helpful message
  if (!fs.existsSync(lambdaBinaryPath)) {
    console.log(`
Lambda binary not found at: ${lambdaBinaryPath}
Please build the Lambda function first with: make lambda
    `);
  }

  // Create a zip archive with the bootstrap binary at the root
  // The binary must be named "bootstrap" at the root of the zip
  const lambdaCode = new pulumi.asset.AssetArchive({
    bootstrap: new pulumi.asset.FileAsset(lambdaBinaryPath),
  });

  // Create Lambda function
  const lambdaFunction = new aws.lambda.Function(
    "rust-lambda-function",
    {
      name: "rust-lambda-function",
      role: lambdaRole.arn,
      runtime: "provided.al2023", // Custom runtime for Rust on Amazon Linux 2023
      handler: "bootstrap", // For custom runtime, this is the executable name
      code: lambdaCode,
      architectures: ["arm64"], // Use ARM64 for better price/performance
      memorySize: 128,
      timeout: 30,
      environment: {
        variables: pulumi
          .all([
            suiPackageId,
            suiChain,
            suiAddress,
            suiSecretKey,
            locksTable.name,
            keypairsTable.name,
            kmsKey.id,
          ])
          .apply(
            ([
              packageId,
              chain,
              address,
              secretKey,
              locksTableName,
              keypairsTableName,
              kmsKeyId,
            ]) => ({
              RUST_BACKTRACE: "1",
              LOG_LEVEL: "info",
              LOCKS_TABLE_NAME: locksTableName,
              KEYPAIRS_TABLE_NAME: keypairsTableName,
              KMS_KEY_ID: kmsKeyId,
              // Sui blockchain configuration
              ...(packageId && { SUI_PACKAGE_ID: packageId }),
              ...(chain && { SUI_CHAIN: chain }),
              ...(address && { SUI_ADDRESS: address }),
              ...(secretKey && { SUI_SECRET_KEY: secretKey }),
            })
          ),
      },
      tags: {
        Name: "rust-lambda-function",
        Language: "Rust",
      },
    },
    {
      dependsOn: [lambdaBasicExecutionPolicy, lambdaCustomPolicy],
    }
  );

  // Create Lambda function URL for easy testing
  const functionUrl = new aws.lambda.FunctionUrl("lambda-function-url", {
    functionName: lambdaFunction.name,
    authorizationType: "NONE", // Change to "AWS_IAM" for production
    cors: {
      allowOrigins: ["*"],
      allowMethods: ["GET", "POST"],
      allowHeaders: ["Content-Type"],
      maxAge: 86400,
    },
  });

  // Optional: Create an API Gateway for the Lambda
  const api = new awsx.classic.apigateway.API("lambda-api", {
    routes: [
      {
        path: "/add",
        method: "POST",
        eventHandler: lambdaFunction,
      },
      {
        path: "/multiply",
        method: "POST",
        eventHandler: lambdaFunction,
      },
      {
        path: "/{proxy+}",
        method: "ANY",
        eventHandler: lambdaFunction,
      },
    ],
  });

  // Return all the created resources
  return {
    bucketName: deploymentBucket.id,
    bucketArn: deploymentBucket.arn,
    lambdaFunctionName: lambdaFunction.name,
    lambdaFunctionArn: lambdaFunction.arn,
    lambdaRoleArn: lambdaRole.arn,
    functionUrl: functionUrl.functionUrl,
    apiUrl: api.url,
    logGroupName: logGroup.name,
    locksTableName: locksTable.name,
    keypairsTableName: keypairsTable.name,
    kmsKeyId: kmsKey.id,
    kmsKeyAlias: kmsKeyAlias.name,
    region: (region as any).name || region.id,
    accountId: accountId.accountId,
  };
};
