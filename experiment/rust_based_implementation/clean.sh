#!/bin/bash

echo "Cleaning up..."
rm -rf node_modules
rm -rf .next
rm -rf dist
# rm -rf src-tauri/target

echo "Installing dependencies..."
# pnpm install

echo "Building Tauri app..."
pnpm run tauri dev
