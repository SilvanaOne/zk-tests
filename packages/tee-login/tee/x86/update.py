#!/usr/bin/env python3
"""
Script to update StageX image SHA256 hashes in Containerfile to their latest ARM64 versions.
"""

import json
import re
import sys
from typing import Dict, List, Tuple, Optional
import requests
import bs4


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
            
        # Match FROM ${STAGEX_REG}/... lines (handles both variable and direct registry formats)
        match = re.match(r'^FROM\s+(?:\$\{STAGEX_REG\}/|[^/]+/)?([^@:]+)[@:]([^@\s]+)\s+AS\s+(.+)$', line)
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
    Fetch the linux/arm64 digest from the StageX package index.
    This avoids registry quirks and works even when the image is hosted
    on Quay or Docker Hub but not GHCR.
    """
    # Handle special cases first
    if image_name == "core-ca-certificates":
        url = "https://stagex.tools/packages/core/ca-certificates"
    elif image_name == "user-linux-nitro":
        url = "https://stagex.tools/packages/user/linux-nitro"
    else:
        # Determine namespace (core vs user) and strip that prefix
        if image_name.startswith("core-"):
            kind = "core"
            subpath = image_name[len("core-"):]
        elif image_name.startswith("user-"):
            kind = "user"
            subpath = image_name[len("user-"):]
        else:
            # Fallback for odd names: treat everything after first dash as subpath
            kind, subpath = image_name.split("-", 1)

        subpath = subpath.replace("-", "/")  # dashes → path separators
        url = f"https://stagex.tools/packages/{kind}/{subpath}"
 
    try:
        print(f"Fetching {url}...")
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()


        # Parse the HTML and look for a line like:  "linux/arm64 sha256:<hex>"
        soup = bs4.BeautifulSoup(resp.text, "html.parser")
        match = soup.find(string=re.compile(r"linux/arm64 sha256:([a-f0-9]{64})"))
        if match:
            return re.search(r"sha256:([a-f0-9]{64})", match).group(1)
        print(f"Warning: no arm64 digest listed for {image_name}")
        return None

    except Exception as e:
        print(f"Error fetching digest for {image_name}: {e}")
        return None


def update_containerfile(containerfile_path: str, updates: Dict[str, str]) -> None:
    """
    Update the Containerfile with new SHA256 hashes.
    """
    with open(containerfile_path, 'r') as f:
        content = f.read()
    
    original_content = content
    
    for image_name, new_hash in updates.items():
        # Pattern to match the FROM line for this image (handles ${STAGEX_REG}/ format)
        pattern = rf'(FROM\s+(?:\$\{{STAGEX_REG\}}/|[^/]+/)?{re.escape(image_name)}@sha256:)[a-f0-9]{{64}}(\s+AS\s+.+)'
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