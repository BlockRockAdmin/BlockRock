#!/bin/bash
set -e
ENV_FILE=~/Documents/BlockRock/.env
if [ ! -f "$ENV_FILE" ]; then
    echo "Error: .env file not found at $ENV_FILE"
    exit 1
fi
source "$ENV_FILE"
export TRONGRIDAPIKEY
export TRON_ADDRESS
cd ~/Documents/BlockRock/zion-core
cargo +nightly run --release --bin node
