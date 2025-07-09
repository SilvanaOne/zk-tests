/**
 * AWS Fargate Infrastructure for Agent Container Execution
 *
 * This Pulumi script creates all the necessary AWS infrastructure to run
 * containers in AWS Fargate instead of local Docker.
 *
 * Usage:
 * 1. Ensure you have AWS credentials configured
 * 2. Run: pulumi up
 * 3. The script will create .env.pulumi with all required environment variables
 * 4. Copy the variables from .env.pulumi to your main .env file
 * 5. Update your Rust code to use fargate.rs instead of docker.rs
 *
 * Infrastructure created:
 * - VPC with public subnets (multi-AZ)
 * - Security group allowing port 6000
 * - ECS Fargate cluster with capacity providers
 * - IAM roles for task execution and application permissions
 * - CloudWatch log group for container logs
 *
 * Environment variables generated:
 * - AWS_REGION
 * - FARGATE_CLUSTER_NAME
 * - FARGATE_SUBNET_IDS (comma-separated)
 * - FARGATE_SECURITY_GROUP_IDS
 * - FARGATE_TASK_ROLE_ARN
 * - FARGATE_EXECUTION_ROLE_ARN
 * - FARGATE_LOG_GROUP_NAME
 * - FARGATE_CPU
 * - FARGATE_MEMORY
 */

import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";
import * as fs from "fs";

async function createFargateInfrastructure() {
  // Get current AWS region
  const region = await aws.getRegion();
  const accountId = await aws.getCallerIdentity();

  // Create VPC with public subnets only (no NAT Gateways to save costs)
  const vpc = new awsx.ec2.Vpc("agent-vpc", {
    numberOfAvailabilityZones: 2,
    enableDnsHostnames: true,
    enableDnsSupport: true,
    // Only create public subnets to avoid NAT Gateway costs
    natGateways: {
      strategy: "None", // No NAT Gateways = saves ~$65/month
    },
    tags: {
      Name: "agent-fargate-vpc",
    },
  });

  // Create security group for Fargate tasks
  const securityGroup = new aws.ec2.SecurityGroup("agent-fargate-sg", {
    vpcId: vpc.vpcId,
    description: "Security group for agent Fargate tasks",
    ingress: [
      {
        fromPort: 6000,
        toPort: 6000,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
        description: "Allow inbound traffic on port 6000",
      },
    ],
    egress: [
      {
        fromPort: 0,
        toPort: 0,
        protocol: "-1",
        cidrBlocks: ["0.0.0.0/0"],
        description: "Allow all outbound traffic",
      },
    ],
    tags: {
      Name: "agent-fargate-security-group",
    },
  });

  // Create CloudWatch log group
  const logGroup = new aws.cloudwatch.LogGroup("agent-log-group", {
    name: "/ecs/agent-tasks",
    retentionInDays: 7,
    tags: {
      Name: "agent-fargate-logs",
    },
  });

  // Create IAM role for task execution
  const executionRole = new aws.iam.Role("agent-execution-role", {
    assumeRolePolicy: JSON.stringify({
      Version: "2012-10-17",
      Statement: [
        {
          Effect: "Allow",
          Principal: {
            Service: "ecs-tasks.amazonaws.com",
          },
          Action: "sts:AssumeRole",
        },
      ],
    }),
    tags: {
      Name: "agent-fargate-execution-role",
    },
  });

  // Attach the execution role policy
  const executionRolePolicyAttachment = new aws.iam.RolePolicyAttachment(
    "agent-execution-role-policy",
    {
      role: executionRole.name,
      policyArn:
        "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy",
    }
  );

  // Additional policy for CloudWatch logs
  const executionRoleLogsPolicy = new aws.iam.RolePolicy(
    "agent-execution-role-logs-policy",
    {
      role: executionRole.id,
      policy: JSON.stringify({
        Version: "2012-10-17",
        Statement: [
          {
            Effect: "Allow",
            Action: [
              "logs:CreateLogStream",
              "logs:PutLogEvents",
              "logs:CreateLogGroup",
            ],
            Resource: "*",
          },
        ],
      }),
    }
  );

  // Create IAM role for task (application permissions)
  const taskRole = new aws.iam.Role("agent-task-role", {
    assumeRolePolicy: JSON.stringify({
      Version: "2012-10-17",
      Statement: [
        {
          Effect: "Allow",
          Principal: {
            Service: "ecs-tasks.amazonaws.com",
          },
          Action: "sts:AssumeRole",
        },
      ],
    }),
    tags: {
      Name: "agent-fargate-task-role",
    },
  });

  // Add policy for task role to access other AWS services if needed
  const taskRolePolicy = new aws.iam.RolePolicy("agent-task-role-policy", {
    role: taskRole.id,
    policy: JSON.stringify({
      Version: "2012-10-17",
      Statement: [
        {
          Effect: "Allow",
          Action: [
            "logs:CreateLogGroup",
            "logs:CreateLogStream",
            "logs:PutLogEvents",
            "logs:DescribeLogStreams",
            "logs:DescribeLogGroups",
          ],
          Resource: "*",
        },
        // Add more permissions here as needed for your application
      ],
    }),
  });

  // Create ECS cluster
  const cluster = new aws.ecs.Cluster("agent-cluster", {
    name: "agent-fargate-cluster",
    tags: {
      Name: "agent-fargate-cluster",
    },
  });

  // Enable CloudWatch Container Insights
  const clusterCapacityProviders = new aws.ecs.ClusterCapacityProviders(
    "agent-cluster-capacity-providers",
    {
      clusterName: cluster.name,
      capacityProviders: ["FARGATE", "FARGATE_SPOT"],
      defaultCapacityProviderStrategies: [
        {
          capacityProvider: "FARGATE",
          weight: 1,
          base: 1,
        },
      ],
    }
  );

  // Wait for all resources to be created and get their values
  const config = pulumi
    .all([
      cluster.name,
      vpc.publicSubnetIds,
      securityGroup.id,
      taskRole.arn,
      executionRole.arn,
      logGroup.name,
      region.name,
    ])
    .apply(
      ([
        clusterName,
        subnetIds,
        sgId,
        taskArn,
        execArn,
        logGroupName,
        regionName,
      ]) => {
        const envContent = `# Generated by Pulumi - AWS Fargate Configuration
AWS_REGION=${regionName}
FARGATE_CLUSTER_NAME=${clusterName}
FARGATE_SUBNET_IDS=${subnetIds.join(",")}
FARGATE_SECURITY_GROUP_IDS=${sgId}
FARGATE_TASK_ROLE_ARN=${taskArn}
FARGATE_EXECUTION_ROLE_ARN=${execArn}
FARGATE_LOG_GROUP_NAME=${logGroupName}
FARGATE_CPU=512
FARGATE_MEMORY=1024
`;

        // Write to .env.pulumi file
        fs.writeFileSync(".env.pulumi", envContent);
        console.log("Environment variables written to .env.pulumi");

        return {
          clusterName,
          subnetIds,
          securityGroupId: sgId,
          taskRoleArn: taskArn,
          executionRoleArn: execArn,
          logGroupName,
          region: regionName,
        };
      }
    );

  // Export the important values
  return {
    vpcId: vpc.vpcId,
    clusterName: cluster.name,
    subnetIds: vpc.publicSubnetIds,
    securityGroupId: securityGroup.id,
    taskRoleArn: taskRole.arn,
    executionRoleArn: executionRole.arn,
    logGroupName: logGroup.name,
    region: region.name,
    accountId: accountId.accountId,
    config: config,
  };
}

// Execute the async function and export results
export const infrastructure = createFargateInfrastructure();

// Export individual values for easy access
export const vpcId = infrastructure.then((infra) => infra.vpcId);
export const clusterName = infrastructure.then((infra) => infra.clusterName);
export const subnetIds = infrastructure.then((infra) => infra.subnetIds);
export const securityGroupId = infrastructure.then(
  (infra) => infra.securityGroupId
);
export const taskRoleArn = infrastructure.then((infra) => infra.taskRoleArn);
export const executionRoleArn = infrastructure.then(
  (infra) => infra.executionRoleArn
);
export const logGroupName = infrastructure.then((infra) => infra.logGroupName);
export const region = infrastructure.then((infra) => infra.region);
export const accountId = infrastructure.then((infra) => infra.accountId);
