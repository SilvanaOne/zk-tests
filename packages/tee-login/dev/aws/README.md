# AWS EC2 Instance Finder

### x86

Filtering instances by 4 vCPUs (limited to 59 instances from file)...
Results sorted by Linux price in us-east-1 region:

## Instance Name Memory Price ($/hr) Processor

t3a.xlarge 16 GiB $0.1504 AMD EPYC 7571
c6a.xlarge 8 GiB $0.1530 AMD EPYC 7R13 Processor
c5a.xlarge 8 GiB $0.1540 AMD EPYC 7R32
t3.xlarge 16 GiB $0.1664 Intel Skylake E5 2686 v5
c5.xlarge 8 GiB $0.1700 Intel Xeon Platinum 8124M
c6i.xlarge 8 GiB $0.1700 Intel Xeon 8375C (Ice Lake)
c5ad.xlarge 8 GiB $0.1720 AMD EPYC 7R32
m5a.xlarge 16 GiB $0.1720 AMD EPYC 7571
m6a.xlarge 16 GiB $0.1728 AMD EPYC 7R13 Processor
c5d.xlarge 8 GiB $0.1920 Intel Xeon Platinum 8124M

Showing 10 of 40 instances found.

### Graviton

Filtering instances by 2 vCPUs (limited to 32 instances from file)...
Results sorted by Linux price in us-east-1 region:

## Instance Name Memory Price ($/hr) Processor

t4g.nano 0.5 GiB $0.0042 AWS Graviton2 Processor
t4g.micro 1 GiB $0.0084 AWS Graviton2 Processor
t4g.small 2 GiB $0.0168 AWS Graviton2 Processor
t4g.medium 4 GiB $0.0336 AWS Graviton2 Processor
a1.large 4 GiB $0.0510 AWS Graviton Processor
t4g.large 8 GiB $0.0672 AWS Graviton2 Processor
c6g.large 4 GiB $0.0680 AWS Graviton2 Processor
c7g.large 4 GiB $0.0725 AWS Graviton3 Processor
c6gd.large 4 GiB $0.0768 AWS Graviton2 Processor
m6g.large 8 GiB $0.0770 AWS Graviton2 Processor

Showing 10 of 17 instances found.

## CLI

export REGION=us-east-1

aws ec2 describe-instance-types --region "$REGION" \
 --filters \
 "Name=vcpu-info.default-vcpus,Values=4" \
 "Name=processor-info.supported-architecture,Values=x86_64" \
 "Name=hypervisor,Values=nitro" \
 --query 'InstanceTypes[].InstanceType' \
 --output text | tr '\t' '\n' > x86.txt

aws ec2 describe-instance-types --region "$REGION" \
 --filters \
 "Name=vcpu-info.default-vcpus,Values=2" \
 "Name=processor-info.supported-architecture,Values=arm64" \
 "Name=hypervisor,Values=nitro" \
 --query 'InstanceTypes[].InstanceType' \
 --output text | tr '\t' '\n' > arm64-2.txt

## Python

A Python program that fetches AWS EC2 instance data and helps you find instances based on CPU count, sorted by Linux pricing in the us-east-1 region.

- Fetches real-time EC2 instance data from [ec2instances.info](https://tedivm.github.io/ec2details/api/ec2instances.json)
- Filters instances by number of vCPUs
- Optionally filters to only instances listed in a file
- Sorts results by Linux pricing in us-east-1 region (prefers Shared pricing)
- Displays instance name, memory, price per hour, and processor type
- Shows top 20 results by default (configurable)

## Requirements

- Python 3.6 or higher
- `requests` library

## Installation

1. Clone or download the files
2. Install dependencies:
   ```bash
   pip install -r requirements.txt
   ```
3. Make the script executable (optional):
   ```bash
   chmod +x aws_instance_finder.py
   ```

## Usage

### Basic Usage

```bash
python3 aws_instance_finder.py <cpu_count>
```

### Examples

Find all instances with 4 vCPUs:

```bash
python3 aws_instance_finder.py 4
```

Find all instances with 8 vCPUs:

```bash
python3 aws_instance_finder.py 8
```

Find all instances with 2 vCPUs:

```bash
python3 aws_instance_finder.py 2
```

### Advanced Options

Show top 10 results instead of 20:

```bash
python3 aws_instance_finder.py 8 --limit 10
```

Filter only instances from a specific file (one instance name per line):

```bash
python3 aws_instance_finder.py 4 --instances-file instances.txt
```

Use a custom data source URL:

```bash
python3 aws_instance_finder.py 4 --url "https://example.com/custom-data.json"
```

### Help

```bash
python3 aws_instance_finder.py --help
```

## Output Format

The program displays results in a table format:

```
Instance Name   Memory       Price ($/hr) Processor
----------------------------------------------------------------------
a1.2xlarge      16 GiB       $0.2040      AWS Graviton Processor
t4g.2xlarge     32 GiB       $0.2688      AWS Graviton2 Processor
c6g.2xlarge     16 GiB       $0.2720      AWS Graviton2 Processor
...

Showing 14 of 14 instances found.
```

## Data Source

The program fetches data from [https://tedivm.github.io/ec2details/api/ec2instances.json](https://tedivm.github.io/ec2details/api/ec2instances.json), which provides comprehensive EC2 instance information including:

- Instance specifications (CPU, memory, storage)
- Pricing across different regions and operating systems
- Processor information
- Network performance
- Current generation status

## Instance File Format

When using the `--instances-file` option, provide a text file with one instance name per line:

```
r6i.xlarge
m6id.xlarge
c6in.xlarge
r6a.xlarge
...
```

Empty lines and leading/trailing whitespace are ignored.

## Filtering Logic

1. **Instance List Filter** (optional): Only includes instances listed in the provided file
2. **CPU Count**: Exact match against the `vcpu` field
3. **Region Availability**: Only includes instances available in us-east-1
4. **Pricing**: Only includes instances with Linux pricing data for us-east-1

## Pricing

- Uses "Shared" pricing when available, falls back to "Dedicated" pricing
- All prices are hourly rates in USD
- Results are sorted by price (lowest to highest)

## Error Handling

The program handles various error conditions:

- Network connectivity issues
- Invalid JSON data
- Missing pricing information
- Invalid command-line arguments

## License

This program is provided as-is for educational and informational purposes.
