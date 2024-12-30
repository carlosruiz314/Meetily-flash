#!/bin/bash

echo "Cleaning up..."
rm -rf node_modules
rm -rf .next
rm -rf dist
rm -rf src-tauri/target

echo "Installing dependencies..."
npm install

echo "Building Tauri app..."
npm run tauri dev
