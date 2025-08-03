#!/bin/bash

# Test script for Sui Move Add contract

echo "Testing Sui Move Add SP1 contract..."

# Change to the contract directory
cd sp1

echo "Building Move contract..."
sui move build

echo "Running Move tests..."
sui move test

echo "Testing completed!"