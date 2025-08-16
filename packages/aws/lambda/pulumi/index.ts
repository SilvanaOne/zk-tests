import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";
import * as path from "path";
import * as fs from "fs";

export = async () => {
  // Get current AWS region
  const region = await aws.getRegion();
  const accountId = await aws.getCallerIdentity();

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
  new aws.s3.BucketVersioning(
    "lambda-deployment-bucket-versioning",
    {
      bucket: deploymentBucket.id,
      versioningConfiguration: {
        status: "Enabled",
      },
    }
  );

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
        }
      ]
    }`,
  });

  // Create CloudWatch Log Group for Lambda
  const logGroup = new aws.cloudwatch.LogGroup("lambda-log-group", {
    name: "/aws/lambda/rust-lambda-function",
    retentionInDays: 7,
    tags: {
      Name: "lambda-logs",
    },
  });

  // Path to the Lambda binary (built with cargo lambda)
  // The actual binary is at target/lambda/bootstrap/bootstrap
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

  // Create a zip archive of the bootstrap binary
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
      memorySize: 512,
      timeout: 30,
      environment: {
        variables: {
          RUST_BACKTRACE: "1",
          LOG_LEVEL: "info",
          // Add any environment variables your Lambda needs
        },
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
        path: "/",
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
    region: (region as any).name || region.id,
    accountId: accountId.accountId,
  };
};
