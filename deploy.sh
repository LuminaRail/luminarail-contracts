#!/usr/bin/env bash

# ==============================================================================
# LuminaRail — Soroban Smart Contract Deployment Script
# ==============================================================================
# Builds, deploys, and initializes LuminaRail smart contracts on Stellar Testnet.
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACTS_DIR="${SCRIPT_DIR}"
BACKEND_ENV_FILE="${CONTRACTS_DIR}/../luminarail-backend/.env"

echo "=================================================="
echo "🚀 LuminaRail Smart Contract Deployment"
echo "=================================================="

if [ -f "$BACKEND_ENV_FILE" ]; then
    echo "📄 Loading environment variables from $BACKEND_ENV_FILE"
    export $(grep -v '^#' "$BACKEND_ENV_FILE" | grep -E 'STELLAR_|SOROBAN_' | xargs)
fi

NETWORK="${STELLAR_NETWORK:-testnet}"
RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
SECRET_KEY="${STELLAR_SETTLEMENT_SIGNER_SECRET_KEY}"
ADMIN_PUBKEY="${STELLAR_SETTLEMENT_SIGNER_PUBLIC_KEY}"

echo "🌐 Target Network: $NETWORK"
echo "🔗 RPC URL:        $RPC_URL"

OPTIMIZE=false
GENERATE_BINDINGS=false

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --optimize) OPTIMIZE=true ;;
        --bindings) GENERATE_BINDINGS=true ;;
        -h|--help)
            echo "Usage: ./deploy.sh [--optimize] [--bindings]"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

if ! command -v stellar &> /dev/null; then
    echo "❌ Error: 'stellar' CLI is not installed."
    echo "Install via: cargo install --locked stellar-cli"
    exit 1
fi

echo "🔧 Using CLI: $(stellar --version | head -n 1)"

# 1. Build Contracts
echo ""
echo "📦 Step 1: Compiling contracts using stellar contract build..."
cd "$CONTRACTS_DIR"
stellar contract build

if [ "$OPTIMIZE" = true ]; then
    if command -v wasm-opt &> /dev/null; then
        echo "⚡ Optimizing WASM binaries with wasm-opt -Oz..."
        for wasm in target/wasm32v1-none/release/*.wasm; do
            if [ -f "$wasm" ]; then
                wasm-opt -Oz "$wasm" -o "$wasm"
                echo "   Optimized $wasm"
            fi
        done
    else
        echo "⚠️ wasm-opt is not installed; skipping optimization."
    fi
fi

if [ "$GENERATE_BINDINGS" = true ]; then
    echo "📜 Generating TypeScript bindings..."
    mkdir -p bindings/ts
    for contract in escrow settlement_vault fee_manager; do
        wasm="target/wasm32v1-none/release/${contract}.wasm"
        if [ -f "$wasm" ]; then
            stellar contract bindings typescript --wasm "$wasm" --output-dir "bindings/ts/${contract}" --overwrite || true
            echo "   Generated TypeScript bindings for ${contract}"
        fi
    done
fi

VAULT_WASM="target/wasm32v1-none/release/settlement_vault.wasm"
ESCROW_WASM="target/wasm32v1-none/release/escrow.wasm"
FEE_WASM="target/wasm32v1-none/release/fee_manager.wasm"

if [ ! -f "$VAULT_WASM" ] || [ ! -f "$ESCROW_WASM" ] || [ ! -f "$FEE_WASM" ]; then
    echo "❌ Error: Contract WASM output missing."
    exit 1
fi

echo "✅ Contract WASM binaries built successfully."

if [ -z "$SECRET_KEY" ]; then
    echo ""
    echo "⚠️ STELLAR_SETTLEMENT_SIGNER_SECRET_KEY is not set."
    echo "   To deploy to $NETWORK, set STELLAR_SETTLEMENT_SIGNER_SECRET_KEY in backend .env."
    exit 0
fi

# 2. Function to deploy
deploy_contract() {
    local wasm_file="$1"
    local name="$2"
    echo "   Deploying $name..." >&2
    local output
    output=$(stellar contract deploy \
        --wasm "$wasm_file" \
        --source-account "$SECRET_KEY" \
        --rpc-url "$RPC_URL" \
        --network-passphrase "$NETWORK_PASSPHRASE" 2>&1)
    
    local contract_id
    contract_id=$(echo "$output" | tail -n 1 | tr -d '[:space:]')
    echo "$contract_id"
}

echo ""
echo "🚀 Step 2: Deploying contracts to $NETWORK..."

VAULT_ID=$(deploy_contract "$VAULT_WASM" "Settlement Vault")
echo "   ✅ Settlement Vault Contract ID: $VAULT_ID"

ESCROW_ID=$(deploy_contract "$ESCROW_WASM" "Escrow")
echo "   ✅ Escrow Contract ID:           $ESCROW_ID"

FEE_ID=$(deploy_contract "$FEE_WASM" "Fee Manager")
echo "   ✅ Fee Manager Contract ID:      $FEE_ID"

# 3. Initialize Contracts
if [ -n "$ADMIN_PUBKEY" ]; then
    echo ""
    echo "⚙️ Step 3: Initializing contracts with admin: $ADMIN_PUBKEY..."
    
    if [ -n "$VAULT_ID" ]; then
        echo "   Initializing Settlement Vault..."
        stellar contract invoke \
            --id "$VAULT_ID" \
            --source-account "$SECRET_KEY" \
            --rpc-url "$RPC_URL" \
            --network-passphrase "$NETWORK_PASSPHRASE" \
            -- initialize --admin "$ADMIN_PUBKEY" || true
    fi

    if [ -n "$FEE_ID" ]; then
        echo "   Initializing Fee Manager (default 25 BPS)..."
        stellar contract invoke \
            --id "$FEE_ID" \
            --source-account "$SECRET_KEY" \
            --rpc-url "$RPC_URL" \
            --network-passphrase "$NETWORK_PASSPHRASE" \
            -- initialize --admin "$ADMIN_PUBKEY" --initial_bps 25 || true
    fi
fi

# 4. Summary & Environment Update
echo ""
echo "=================================================="
echo "🎉 Deployment Complete!"
echo "=================================================="
echo "SOROBAN_SETTLEMENT_VAULT_CONTRACT_ID=$VAULT_ID"
echo "SOROBAN_ESCROW_CONTRACT_ID=$ESCROW_ID"
echo "SOROBAN_FEE_MANAGER_CONTRACT_ID=$FEE_ID"
echo "=================================================="

if [ -f "$BACKEND_ENV_FILE" ] && [ -n "$VAULT_ID" ]; then
    echo "Updating $BACKEND_ENV_FILE..."
    sed -i "s|^SOROBAN_SETTLEMENT_VAULT_CONTRACT_ID=.*|SOROBAN_SETTLEMENT_VAULT_CONTRACT_ID=$VAULT_ID|" "$BACKEND_ENV_FILE"
    sed -i "s|^SOROBAN_ESCROW_CONTRACT_ID=.*|SOROBAN_ESCROW_CONTRACT_ID=$ESCROW_ID|" "$BACKEND_ENV_FILE"
    sed -i "s|^SOROBAN_FEE_MANAGER_CONTRACT_ID=.*|SOROBAN_FEE_MANAGER_CONTRACT_ID=$FEE_ID|" "$BACKEND_ENV_FILE"
    echo "✅ backend .env updated successfully."
fi
