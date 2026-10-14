#!/usr/bin/env bash
set -e

# 1. Install Rust wasm32 target
rustup target add wasm32-unknown-unknown

# 2. Download prebuilt Trunk binary directly
TRUNK_VERSION="v0.21.5"
mkdir -p .bin
if [ ! -f .bin/trunk ]; then
  echo "Downloading precompiled Trunk..."
  curl -sL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xz -C .bin
fi

# 3. Build release bundle into dist/
./.bin/trunk build --release
