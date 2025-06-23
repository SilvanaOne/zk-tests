import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";
import * as fs from "fs";

export = async () => {
  // Create an AWS resource (S3 Bucket)
  const bucket = new aws.s3.BucketV2("silvana-tee-login");

  const table = new aws.dynamodb.Table("silvana-tee-login", {
    attributes: [{ name: "id", type: "B" }],
    hashKey: "id",
    billingMode: "PAY_PER_REQUEST",
  });

  const api = new aws.iam.User("silvana-tee-api-user", {
    name: "silvana-tee-api-user",
  });

  // Create IAM policy for S3 access
  const s3Policy = new aws.iam.Policy("silvana-tee-s3-policy", {
    description: "Policy for S3 bucket access",
    policy: pulumi.all([bucket.arn]).apply(([bucketArn]) =>
      JSON.stringify({
        Version: "2012-10-17",
        Statement: [
          {
            Effect: "Allow",
            Action: [
              "s3:GetObject",
              "s3:PutObject",
              "s3:DeleteObject",
              "s3:ListBucket",
              "s3:GetBucketLocation",
            ],
            Resource: [bucketArn, `${bucketArn}/*`],
          },
        ],
      })
    ),
  });

  // Create IAM policy for DynamoDB access
  const dynamoPolicy = new aws.iam.Policy("silvana-tee-dynamo-policy", {
    description: "Policy for DynamoDB table access",
    policy: pulumi.all([table.arn]).apply(([tableArn]) =>
      JSON.stringify({
        Version: "2012-10-17",
        Statement: [
          {
            Effect: "Allow",
            Action: [
              "dynamodb:GetItem",
              "dynamodb:PutItem",
              "dynamodb:UpdateItem",
              "dynamodb:DeleteItem",
              "dynamodb:Query",
              "dynamodb:Scan",
              "dynamodb:BatchGetItem",
              "dynamodb:BatchWriteItem",
            ],
            Resource: tableArn,
          },
        ],
      })
    ),
  });

  // Attach policies to user
  const s3PolicyAttachment = new aws.iam.UserPolicyAttachment(
    "s3-policy-attachment",
    {
      user: api.name,
      policyArn: s3Policy.arn,
    }
  );

  const dynamoPolicyAttachment = new aws.iam.UserPolicyAttachment(
    "dynamo-policy-attachment",
    {
      user: api.name,
      policyArn: dynamoPolicy.arn,
    }
  );

  // Create access keys for the user
  const apiAccessKey = new aws.iam.AccessKey("silvana-tee-api-key", {
    user: api.name,
  });

  const amiId = "ami-085ad6ae776d8f09c"; // x86_64
  const amiIdArm64 = (
    await aws.ssm.getParameter({
      name: "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64",
    })
  ).value;
  const keyPairName = "TEE"; // TODO: create key pair in AWS
  const kmsKeyName = "TEEKMS"; // TODO: create kms key in AWS

  // Get KMS key by alias and create policy
  const kmsKey = await aws.kms.getAlias({
    name: `alias/${kmsKeyName}`,
  });

  // Create IAM policy for KMS access
  const kmsPolicy = new aws.iam.Policy("silvana-tee-kms-policy", {
    description: "Policy for KMS key access with TEE attestation conditions",
    policy: pulumi.output(kmsKey).apply((key) =>
      JSON.stringify({
        Version: "2012-10-17",
        Statement: [
          {
            Effect: "Allow",
            Action: ["kms:Decrypt", "kms:GenerateDataKey*"],
            Resource: key.targetKeyArn,
            /*
                #95 0.797 BootMeasurement: Sha384 { ... }: {"HashAlgorithm": "Sha384 { ... }",
                "PCR0": "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883",
                "PCR1": "b9d08361baa85f592c98b491f1982caaf03f5b1fb8a2a76452f5754510c6864dc88cfa146d43704c9ff9911a2b822883", 
                "PCR2": "21b9efbc184807662e966d34f390821309eeac6802309798826296bf3e8bec7c10edb30948c90ba67310f7b964fc500a"}
                #95 DONE 0.8s
            */
            // Condition: {
            //   StringEqualsIgnoreCase: {
            //     "kms:RecipientAttestation:ImageSha384":
            //       "0ddb90f647b8735da8f9bee096ad7589b2de2f69f5b5a530c1c1c6aee1018eb09db706d6df8bc910b62f07c79db196a1",
            //     // "kms:RecipientAttestation:PCR1":
            //     //   "0x6522d6093479ba18f09bff60f67f0f2e48876c4d757b4bbdeec336edb38a15a8335c3924eeaf923a7dd20a5e064de5f6",
            //     // "kms:RecipientAttestation:PCR2":
            //     //   "0x21b9efbc184807662e966d34f390821309eeac6802309798826296bf3e8bec7c10edb30948c90ba67310f7b964fc500a",
            //   },
            // },
          },
        ],
      })
    ),
  });

  // Attach KMS policy to API user
  const kmsPolicyAttachment = new aws.iam.UserPolicyAttachment(
    "kms-policy-attachment",
    {
      user: api.name,
      policyArn: kmsPolicy.arn,
    }
  );

  // Create Elastic IP
  const elasticIp = new aws.ec2.Eip("silvana-tee-login-ip", {
    domain: "vpc",
    tags: {
      Name: "silvana-tee-login-ip",
      Project: "silvana-tee-login",
    },
  });

  // Create Security Group
  const securityGroup = new aws.ec2.SecurityGroup("silvana-tee-login-sg", {
    name: "silvana-tee-login-sg",
    description: "Security group allowing SSH (22), HTTPS (443), and port 3000",
    ingress: [
      {
        description: "SSH",
        fromPort: 22,
        toPort: 22,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "HTTPS",
        fromPort: 443,
        toPort: 443,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "Port 3000",
        fromPort: 3000,
        toPort: 3000,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "Port 80",
        fromPort: 80,
        toPort: 80,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
    ],
    egress: [
      {
        description: "All outbound traffic",
        fromPort: 0,
        toPort: 0,
        protocol: "-1",
        cidrBlocks: ["0.0.0.0/0"],
      },
    ],
    tags: {
      Name: "silvana-tee-login-sg",
      Project: "silvana-tee-login",
    },
  });

  // Create IAM role for EC2 instance
  const ec2Role = new aws.iam.Role("silvana-tee-ec2-role", {
    name: "silvana-tee-ec2-role",
    assumeRolePolicy: JSON.stringify({
      Version: "2012-10-17",
      Statement: [
        {
          Action: "sts:AssumeRole",
          Effect: "Allow",
          Principal: {
            Service: "ec2.amazonaws.com",
          },
        },
      ],
    }),
  });

  // Attach the same S3 and DynamoDB policies to the EC2 role
  const ec2S3PolicyAttachment = new aws.iam.RolePolicyAttachment(
    "ec2-s3-policy-attachment",
    {
      role: ec2Role.name,
      policyArn: s3Policy.arn,
    }
  );

  const ec2DynamoPolicyAttachment = new aws.iam.RolePolicyAttachment(
    "ec2-dynamo-policy-attachment",
    {
      role: ec2Role.name,
      policyArn: dynamoPolicy.arn,
    }
  );

  const ec2KmsPolicyAttachment = new aws.iam.RolePolicyAttachment(
    "ec2-kms-policy-attachment",
    {
      role: ec2Role.name,
      policyArn: kmsPolicy.arn,
    }
  );

  const s3images = "silvana-tee-images";

  // Get existing S3 bucket
  const s3imagesBucket = await aws.s3.getBucket({
    bucket: s3images,
  });

  // Create IAM policy for existing S3 bucket access
  const s3ImagesPolicy = new aws.iam.Policy("silvana-tee-s3-images-policy", {
    description: "Policy for existing S3 bucket access",
    policy: pulumi.output(s3imagesBucket).apply((bucket) =>
      JSON.stringify({
        Version: "2012-10-17",
        Statement: [
          {
            Effect: "Allow",
            Action: [
              "s3:GetObject",
              "s3:PutObject",
              "s3:DeleteObject",
              "s3:ListBucket",
              "s3:GetBucketLocation",
            ],
            Resource: [bucket.arn, `${bucket.arn}/*`],
          },
        ],
      })
    ),
  });

  // Attach S3 images policy to EC2 role
  const ec2S3ImagesPolicyAttachment = new aws.iam.RolePolicyAttachment(
    "ec2-s3-images-policy-attachment",
    {
      role: ec2Role.name,
      policyArn: s3ImagesPolicy.arn,
    }
  );

  // Create instance profile for the EC2 instance
  const instanceProfile = new aws.iam.InstanceProfile(
    "silvana-tee-instance-profile",
    {
      name: "silvana-tee-instance-profile",
      role: ec2Role.name,
    }
  );

  // Create EC2 Instance
  // c7g.4xlarge - Graviton 0.58 per hour, 16 cpu
  // c6a.xlarge - min Intel, 4 cpu, 16gb ram
  const instance = new aws.ec2.Instance("silvana-tee-login-instance", {
    ami: amiId,
    instanceType: "c6a.xlarge", //"m5.xlarge",  minimum:  or t4g.nano ($0.0042 per hour), standard: m5.xlarge or m5.2xlarge, good: c7i.4xlarge "c7g.4xlarge"
    keyName: keyPairName,
    vpcSecurityGroupIds: [securityGroup.id],
    iamInstanceProfile: instanceProfile.name,

    // Enable Nitro Enclaves
    enclaveOptions: {
      enabled: true,
    },

    rootBlockDevice: {
      volumeSize: 30,
      volumeType: "gp3",
      deleteOnTermination: true,
    },

    // User data script loaded from user-data.sh file
    userData: fs.readFileSync("./user-data.sh", "utf8"),
    userDataReplaceOnChange: false,

    tags: {
      Name: "silvana-tee-login-instance",
      Project: "silvana-tee-login",
      "instance-script": "true",
    },
  });

  // Associate Elastic IP with the instance
  const eipAssociation = new aws.ec2.EipAssociation(
    "silvana-tee-login-eip-association",
    {
      instanceId: instance.id,
      allocationId: elasticIp.allocationId,
    }
  );

  // Create another Elastic IP for arm instance
  const armElasticIp = new aws.ec2.Eip("silvana-tee-login-arm-ip", {
    domain: "vpc",
    tags: {
      Name: "silvana-tee-login-arm-ip",
      Project: "silvana-tee-login",
    },
  });

  // Create arm EC2 Instance with ARM64 and larger disk
  const armInstance = new aws.ec2.Instance("silvana-tee-login-arm-instance", {
    ami: amiIdArm64,
    instanceType: "c6g.large",
    keyName: keyPairName,
    vpcSecurityGroupIds: [securityGroup.id],
    iamInstanceProfile: instanceProfile.name,

    // Enable Nitro Enclaves
    enclaveOptions: {
      enabled: true,
    },

    rootBlockDevice: {
      volumeSize: 20,
      volumeType: "gp3",
      deleteOnTermination: true,
    },

    // User data script loaded from user-data.sh file
    userData: fs.readFileSync("./user-data.sh", "utf8"),
    userDataReplaceOnChange: false,

    tags: {
      Name: "silvana-tee-login-arm-instance",
      Project: "silvana-tee-login",
      "instance-script": "true",
    },
  });

  // Associate arm Elastic IP with the arm instance
  const armEipAssociation = new aws.ec2.EipAssociation(
    "silvana-tee-login-arm-eip-association",
    {
      instanceId: armInstance.id,
      allocationId: armElasticIp.allocationId,
    }
  );

  // Return all outputs
  return {
    bucketName: bucket.id,
    tableName: table.id,
    apiAccessKeyId: apiAccessKey.id,
    apiSecretKey: apiAccessKey.secret,
    kmsKeyArn: pulumi.output(kmsKey).apply((k) => k.targetKeyArn),
    elasticIpId: elasticIp.id,
    elasticIpAddress: elasticIp.publicIp,
    elasticIpAllocationId: elasticIp.allocationId,
    securityGroupId: securityGroup.id,
    securityGroupName: securityGroup.name,
    kmsPolicyArn: kmsPolicy.arn,
    amiIdX86: amiId,
    amiIdArm64: amiIdArm64,
    instanceId: instance.id,
    instancePublicIp: elasticIp.publicIp,
    instancePrivateIp: instance.privateIp,
    ec2RoleArn: ec2Role.arn,
    instanceProfileArn: instanceProfile.arn,
    s3imagesBucketName: s3imagesBucket.id,
    s3imagesBucketArn: s3imagesBucket.arn,
    // arm instance outputs
    armElasticIpAddress: armElasticIp.publicIp,
    armInstanceId: armInstance.id,
    armInstancePublicIp: armElasticIp.publicIp,
    armInstancePrivateIp: armInstance.privateIp,
  };
};
