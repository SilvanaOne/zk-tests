#!/usr/bin/env python3
"""
Script to update StageX image SHA256 hashes in Containerfile to their latest ARM64 versions.
"""

import json
import re
import subprocess
import sys
from typing import Dict, List, Tuple, Optional


def extract_stagex_images(containerfile_path: str) -> List[Tuple[str, str, str]]:
    """
    Extract stagex images from Containerfile.
    Returns list of (image_name, current_hash, full_line) tuples.
    """
    images = []
    
    with open(containerfile_path, 'r') as f:
        lines = f.readlines()
    
    for i, line in enumerate(lines):
        line = line.strip()
        
        # Skip comments and empty lines
        if line.startswith('#') or not line:
            continue
            
        # Match FROM stagex/... lines
        match = re.match(r'^FROM\s+stagex/([^@:]+)[@:]([^@\s]+)\s+AS\s+(.+)$', line)
        if match:
            image_name = match.group(1)
            current_ref = match.group(2)  # Could be sha256:hash or 'local'
            alias = match.group(3)
            
            # Skip local images for now
            if current_ref == 'local':
                print(f"Skipping local image: {image_name}")
                continue
                
            # Extract current hash if it's a SHA256
            if current_ref.startswith('sha256:'):
                current_hash = current_ref[7:]  # Remove 'sha256:' prefix
                images.append((image_name, current_hash, line))
            
    return images


def get_arm64_hash(image_name: str) -> Optional[str]:
    """
    Get the ARM64 SHA256 hash for a stagex image using docker manifest inspect.
    """
    registry_url = f"ghcr.io/siderolabs/stagex/{image_name}:latest"
    
    try:
        print(f"Fetching manifest for {image_name}...")
        result = subprocess.run(
            ['docker', 'manifest', 'inspect', '--verbose', registry_url],
            capture_output=True,
            text=True,
            check=True
        )
        
        manifest_data = json.loads(result.stdout)
        
        # Look for ARM64 platform
        for entry in manifest_data:
            descriptor = entry.get('Descriptor', {})
            platform = descriptor.get('platform', {})
            
            if platform.get('architecture') == 'arm64' and platform.get('os') == 'linux':
                # Extract SHA256 from the Ref field
                ref = entry.get('Ref', '')
                match = re.search(r'@sha256:([a-f0-9]{64})', ref)
                if match:
                    return match.group(1)
        
        print(f"Warning: No ARM64 platform found for {image_name}")
        return None
        
    except subprocess.CalledProcessError as e:
        print(f"Error fetching manifest for {image_name}: {e}")
        print(f"stderr: {e.stderr}")
        return None
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON for {image_name}: {e}")
        return None


def update_containerfile(containerfile_path: str, updates: Dict[str, str]) -> None:
    """
    Update the Containerfile with new SHA256 hashes.
    """
    with open(containerfile_path, 'r') as f:
        content = f.read()
    
    original_content = content
    
    for image_name, new_hash in updates.items():
        # Pattern to match the FROM line for this image
        pattern = rf'(FROM\s+stagex/{re.escape(image_name)}@sha256:)[a-f0-9]{{64}}(\s+AS\s+.+)'
        replacement = rf'\g<1>{new_hash}\g<2>'
        
        content = re.sub(pattern, replacement, content)
    
    if content != original_content:
        # Create backup
        backup_path = f"{containerfile_path}.backup"
        with open(backup_path, 'w') as f:
            f.write(original_content)
        print(f"Backup created: {backup_path}")
        
        # Write updated content
        with open(containerfile_path, 'w') as f:
            f.write(content)
        print(f"Updated {containerfile_path}")
    else:
        print("No changes needed.")


def main():
    containerfile_path = "Containerfile"
    
    if len(sys.argv) > 1:
        containerfile_path = sys.argv[1]
    
    print(f"Processing {containerfile_path}...")
    
    # Extract current images
    images = extract_stagex_images(containerfile_path)
    print(f"Found {len(images)} stagex images to update")
    
    # Fetch new hashes
    updates = {}
    failed_images = []
    
    for image_name, current_hash, full_line in images:
        print(f"\nProcessing {image_name}...")
        print(f"Current hash: {current_hash}")
        
        new_hash = get_arm64_hash(image_name)
        if new_hash:
            if new_hash != current_hash:
                print(f"New hash:     {new_hash} ✓ (UPDATED)")
                updates[image_name] = new_hash
            else:
                print(f"New hash:     {new_hash} (no change)")
        else:
            print(f"Failed to get new hash for {image_name}")
            failed_images.append(image_name)
    
    # Update the file
    if updates:
        print(f"\nUpdating {len(updates)} images in {containerfile_path}...")
        update_containerfile(containerfile_path, updates)
    else:
        print("\nNo updates needed.")
    
    if failed_images:
        print(f"\nWarning: Failed to update {len(failed_images)} images:")
        for img in failed_images:
            print(f"  - {img}")
        sys.exit(1)
    
    print("\n✅ All images processed successfully!")


if __name__ == "__main__":
    main() 