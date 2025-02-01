#!/bin/bash

# Exit on error
set -e

# Clean up previous builds
echo "Cleaning up previous builds..."
rm -rf target/
rm -rf src-tauri/target
rm -rf src-tauri/gen

# Clean up npm, pnp and next
echo "Cleaning up npm, pnp and next..."
rm -rf node_modules
rm -rf .next
rm -rf .pnp.cjs

echo "Installing dependencies..."
pnpm install


# # Set up trap for cleanup
trap cleanup EXIT INT TERM



echo "Building Tauri app..."
pnpm run tauri dev

sleep