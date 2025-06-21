#!/usr/bin/env python3
"""
AWS EC2 Instance Finder

This script fetches AWS EC2 instance data and filters instances based on:
- Number of vCPUs
- Optional instance list from file
- Sorts by Linux pricing in us-east-1 region
- Shows top 20 instances with name, memory, and price
"""

import requests
import json
import argparse
import sys
from typing import List, Dict, Any, Optional, Set


def fetch_instance_data(url: str) -> Dict[str, Any]:
    """Fetch EC2 instance data from the provided URL."""
    try:
        response = requests.get(url)
        response.raise_for_status()
        return response.json()
    except requests.RequestException as e:
        print(f"Error fetching data: {e}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON data: {e}", file=sys.stderr)
        sys.exit(1)


def load_instance_list(filename: str) -> Set[str]:
    """Load instance names from a text file, one per line."""
    try:
        with open(filename, 'r') as f:
            # Read lines, strip whitespace, and filter out empty lines
            instances = {line.strip() for line in f if line.strip()}
        print(f"Loaded {len(instances)} instances from {filename}")
        return instances
    except FileNotFoundError:
        print(f"Error: File '{filename}' not found.", file=sys.stderr)
        sys.exit(1)
    except IOError as e:
        print(f"Error reading file '{filename}': {e}", file=sys.stderr)
        sys.exit(1)


def get_linux_price_us_east_1(instance_data: Dict[str, Any]) -> Optional[float]:
    """Extract Linux price for us-east-1 region (using Shared pricing)."""
    try:
        prices = instance_data.get('prices', {})
        linux_prices = prices.get('Linux', {})
        us_east_1_prices = linux_prices.get('us-east-1', {})
        
        # Prefer Shared pricing, fall back to Dedicated if Shared not available
        if 'Shared' in us_east_1_prices:
            return float(us_east_1_prices['Shared'])
        elif 'Dedicated' in us_east_1_prices:
            return float(us_east_1_prices['Dedicated'])
        else:
            return None
    except (KeyError, ValueError, TypeError):
        return None


def filter_instances(
    instances: Dict[str, Any], 
    cpu_count: int,
    allowed_instances: Optional[Set[str]] = None
) -> List[tuple]:
    """
    Filter instances based on CPU count.
    Returns list of tuples: (instance_name, instance_data, price)
    """
    filtered_instances = []
    
    for instance_name, instance_data in instances.items():
        # Check if instance is in the allowed list (if provided)
        if allowed_instances is not None and instance_name not in allowed_instances:
            continue
        
        # Check if instance has the required number of vCPUs
        vcpu = instance_data.get('vcpu')
        if vcpu != cpu_count:
            continue
        
        # Check if instance has Linux pricing for us-east-1
        price = get_linux_price_us_east_1(instance_data)
        if price is None:
            continue
        
        # Check if us-east-1 is in the available regions
        regions = instance_data.get('regions', [])
        if 'us-east-1' not in regions:
            continue
        
        filtered_instances.append((instance_name, instance_data, price))
    
    return filtered_instances


def display_results(filtered_instances: List[tuple], limit: int = 20):
    """Display the filtered and sorted instances."""
    if not filtered_instances:
        print("No instances found matching the criteria.")
        return
    
    # Sort by price (ascending)
    sorted_instances = sorted(filtered_instances, key=lambda x: x[2])
    
    # Display header
    print(f"{'Instance Name':<15} {'Memory':<12} {'Price ($/hr)':<12} {'Processor'}")
    print("-" * 70)
    
    # Display top instances (up to limit)
    for instance_name, instance_data, price in sorted_instances[:limit]:
        memory = instance_data.get('memory', 'N/A')
        processor = instance_data.get('physicalProcessor', 'N/A')
        
        # Truncate processor name if too long
        if len(processor) > 30:
            processor = processor[:27] + "..."
        
        print(f"{instance_name:<15} {memory:<12} ${price:<11.4f} {processor}")
    
    # Show summary
    total_found = len(sorted_instances)
    shown = min(limit, total_found)
    print(f"\nShowing {shown} of {total_found} instances found.")


def main():
    """Main function to run the AWS instance finder."""
    parser = argparse.ArgumentParser(
        description="Find AWS EC2 instances by CPU count"
    )
    parser.add_argument(
        "cpu_count", 
        type=int, 
        help="Number of vCPUs required"
    )
    parser.add_argument(
        "--limit", 
        type=int, 
        default=20, 
        help="Maximum number of instances to display (default: 20)"
    )
    parser.add_argument(
        "--url", 
        default="https://tedivm.github.io/ec2details/api/ec2instances.json",
        help="URL to fetch EC2 instance data from"
    )
    parser.add_argument(
        "--instances-file", 
        help="File containing instance names to filter by (one per line)"
    )
    
    args = parser.parse_args()
    
    # Load allowed instances if file is provided
    allowed_instances = None
    if args.instances_file:
        allowed_instances = load_instance_list(args.instances_file)
    
    print(f"Fetching EC2 instance data...")
    instances = fetch_instance_data(args.url)
    
    filter_msg = f"Filtering instances by {args.cpu_count} vCPUs"
    if allowed_instances:
        filter_msg += f" (limited to {len(allowed_instances)} instances from file)"
    filter_msg += "..."
    print(filter_msg)
    
    filtered_instances = filter_instances(instances, args.cpu_count, allowed_instances)
    
    print(f"Results sorted by Linux price in us-east-1 region:\n")
    display_results(filtered_instances, args.limit)


if __name__ == "__main__":
    main() 