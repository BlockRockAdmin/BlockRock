#!/bin/bash
set -e

# Determine the directory where this script resides
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Allow overriding the location of the .env file via BLOCKROCK_ENV
ENV_FILE="${BLOCKROCK_ENV:-"$SCRIPT_DIR/.env"}"

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: .env file not found at $ENV_FILE"
    exit 1
fi

# Load environment variables required by the node
source "$ENV_FILE"
export TRONGRIDAPIKEY
export TRON_ADDRESS

# Run the node from the zion-core directory relative to the script
cd "$SCRIPT_DIR/zion-core"
cargo +nightly run --release --bin node
