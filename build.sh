#!/usr/bin/env bash
set -e

echo "=== 1. Checking / Installing Rust ==="
if ! command -v rustup &> /dev/null; then
  echo "Rustup not found. Installing Rust toolchain..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  source "$HOME/.cargo/env"
fi

# Ensure cargo and rustup binaries are in the current PATH
export PATH="$HOME/.cargo/bin:$PATH"

echo "Using Rust: $(rustc --version)"

echo "=== 2. Adding WebAssembly Target ==="
rustup target add wasm32-unknown-unknown

echo "=== 3. Setting up Trunk ==="
TRUNK_VERSION="v0.21.5"
mkdir -p .bin
if [ ! -f .bin/trunk ]; then
  echo "Downloading precompiled Trunk ${TRUNK_VERSION}..."
  curl -sL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xz -C .bin
fi
chmod +x .bin/trunk

echo "=== 4. Ensuring Headers File Exists ==="
mkdir -p static
if [ ! -f static/_headers ]; then
  cat << 'EOF' > static/_headers
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
  X-Content-Type-Options: nosniff
EOF
fi

echo "=== 5. Building Release via Trunk ==="
./.bin/trunk build --release

echo "=== Build Complete: dist/ ready ==="
