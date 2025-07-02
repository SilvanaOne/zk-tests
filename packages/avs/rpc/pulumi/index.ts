import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";
import * as fs from "fs";

export = async () => {
  // Get ARM64 AMI for eu-central-1 region
  const amiIdArm64 = (
    await aws.ssm.getParameter({
      name: "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64",
    })
  ).value;

  const keyPairName = "RPC"; // Using RPC keypair
  const region = "eu-central-1"; // Using eu-central-1 region

  // Create Elastic IP for the RPC instance
  const rpcElasticIp = new aws.ec2.Eip("silvana-rpc-ip", {
    domain: "vpc",
    tags: {
      Name: "silvana-rpc-ip",
      Project: "silvana-rpc",
    },
  });

  // Create Security Group allowing SSH, HTTPS, gRPC, NATS, and monitoring ports
  const securityGroup = new aws.ec2.SecurityGroup("silvana-rpc-sg", {
    name: "silvana-rpc-sg",
    description:
      "Security group for Silvana RPC: SSH (22), HTTP (80), HTTPS (443), gRPC (50051), NATS (4222), NATS-WS (8080), NATS monitoring (8222), Prometheus metrics (9090)",
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
        description: "gRPC Port 50051",
        fromPort: 50051,
        toPort: 50051,
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
      {
        description: "NATS Port 4222",
        fromPort: 4222,
        toPort: 4222,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "NATS WebSocket Port 8080",
        fromPort: 8080,
        toPort: 8080,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "NATS Monitoring Port 8222",
        fromPort: 8222,
        toPort: 8222,
        protocol: "tcp",
        cidrBlocks: ["0.0.0.0/0"],
      },
      {
        description: "Prometheus Metrics Port 9090",
        fromPort: 9090,
        toPort: 9090,
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
      Name: "silvana-rpc-sg",
      Project: "silvana-rpc",
    },
  });

  // Create EC2 Instance with Graviton c7g.4xlarge (without Nitro Enclaves)
  const rpcInstance = new aws.ec2.Instance(
    "silvana-rpc-instance",
    {
      ami: amiIdArm64,
      instanceType: "c7g.4xlarge", // Graviton processor as requested
      keyName: keyPairName,
      vpcSecurityGroupIds: [securityGroup.id],

      // NO Nitro Enclaves enabled (removed enclaveOptions)

      rootBlockDevice: {
        volumeSize: 100,
        volumeType: "gp3",
        deleteOnTermination: true,
      },

      // User data script loaded from user-data.sh file
      userData: fs.readFileSync("./user-data.sh", "utf8"),
      userDataReplaceOnChange: true,

      tags: {
        Name: "silvana-rpc-instance",
        Project: "silvana-rpc",
        "instance-script": "true",
      },
    },
    {
      //ignoreChanges: ["userData"],
    }
  );

  // Associate Elastic IP with the instance
  const eipAssociation = new aws.ec2.EipAssociation(
    "silvana-rpc-eip-association",
    {
      instanceId: rpcInstance.id,
      allocationId: rpcElasticIp.allocationId,
    }
  );

  // Return all outputs
  return {
    rpcElasticIpAddress: rpcElasticIp.publicIp,
    securityGroupId: securityGroup.id,
    amiIdArm64: amiIdArm64,
    rpcInstanceId: rpcInstance.id,
    rpcInstancePublicIp: rpcElasticIp.publicIp,
    rpcInstancePrivateIp: rpcInstance.privateIp,
    region: region,
    keyPairName: keyPairName,
  };
};
