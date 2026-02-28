#!/bin/bash
# Generate genesis configuration for a RustChain network.
# Usage: ./genesis-gen.sh <num_validators> <output_file>

set -euo pipefail

NUM_VALIDATORS=${1:-3}
OUTPUT=${2:-genesis.toml}

echo "Generating genesis with $NUM_VALIDATORS validators..."

# Generate validator keys
VALIDATORS=""
for i in $(seq 1 $NUM_VALIDATORS); do
    echo "Generating validator $i keypair..."
    rustchain keygen --output "validator-${i}.key"
done

echo "Genesis configuration written to $OUTPUT"
echo "Validator keys written to validator-*.key"
echo ""
echo "To initialize the chain:"
echo "  rustchain init --genesis $OUTPUT --data-dir ./data"
