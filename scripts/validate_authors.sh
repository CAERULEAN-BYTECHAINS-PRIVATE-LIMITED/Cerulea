#!/usr/bin/env bash
# ==============================================================================
# validate_authors.sh — Compare runtime expected-author vs actual block digest author
# for a continuous run of blocks, printing any mismatch immediately.
# ==============================================================================

RPC="${1:-http://localhost:9944}"
BLOCKS="${2:-30}"   # how many consecutive blocks to observe
POLL_INTERVAL=1     # seconds between polls

# ---- helpers -----------------------------------------------------------------
rpc() {
    curl -s --connect-timeout 5 -X POST "$RPC" \
         -H "Content-Type: application/json" \
         -d "$1"
}

hex_to_dec() {
    python3 -c "print(int('$1', 16))"
}

# Get current best block number
get_best_number() {
    local resp
    resp=$(rpc '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}')
    local num
    num=$(echo "$resp" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['result']['number'])" 2>/dev/null)
    echo "$num"
}

# Get block hash at a given block number
get_block_hash() {
    local num=$1
    local resp
    resp=$(rpc "{\"jsonrpc\":\"2.0\",\"method\":\"chain_getBlockHash\",\"params\":[$num],\"id\":1}")
    echo "$resp" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['result'])" 2>/dev/null
}

# Get the full block (header + extrinsics) for a given hash
get_block() {
    local hash=$1
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"chain_getBlock\",\"params\":[\"$hash\"],\"id\":1}"
}

# Call dcf_getExpectedAuthor via runtime API
get_expected_author() {
    local hash=$1
    local block_num=$2
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"state_call\",\"params\":[\"DcfApi_get_expected_author\",\"$(printf '%08x' "$block_num" | fold -w2 | tac | tr -d '\n')\",\"$hash\"],\"id\":1}" 2>/dev/null
}

# Extract the CBC digest log from a block header
# Digest log format: 0x0663626364 <32-byte AccountId>
# 0x06 = PreRuntime, "cbcd" engine
extract_digest_author() {
    local digest_hex="$1"
    # digest_hex looks like: 0x06636263638088dc3417...
    # bytes: [type=06][engine=63626364 = "cbcd"][data...]
    # We need bytes [5..37) — the 32-byte account id
    # Strip 0x, verify prefix 0x0663626364
    local stripped="${digest_hex#0x}"
    local prefix4="${stripped:0:10}"   # 5 bytes = 10 hex chars: type + engine
    if [[ "${prefix4:0:2}" == "06" ]]; then
        # PreRuntime digest — account starts after type(1B) + engine_id(4B) = 5B = 10 hex
        # But Substrate encodes length before data, so: type(1) + engine(4) + scale-len + data
        # For a 32-byte account: SCALE-compact(32) = 0x80 (1 byte) → offset = 5+1=6 bytes = 12 hex chars
        local account_hex="${stripped:12:64}"
        echo "0x$account_hex"
    else
        echo "unknown"
    fi
}

# Decode the SCALE-encoded Option<AccountId> returned from state_call
# Option<AccountId>: 0x00 = None, 0x01 + 32 bytes = Some(account)
decode_option_account_id() {
    local hex="$1"
    local stripped="${hex#0x}"
    if [[ "${stripped:0:2}" == "01" ]]; then
        echo "0x${stripped:2:64}"
    elif [[ "${stripped:0:2}" == "00" ]]; then
        echo "NONE"
    else
        echo "DECODE_ERROR:$hex"
    fi
}

# ---- main loop ---------------------------------------------------------------
echo "============================================================"
echo "  CBC Author Selection Validator"
echo "  RPC: $RPC"
echo "  Observing $BLOCKS consecutive blocks"
echo "============================================================"
printf "%-8s %-68s %-68s %s\n" "Block#" "Expected Author (runtime)" "Actual Author (header digest)" "Match?"
echo "----------------------------------------------------------------------------------------------------------------------------------------"

MATCH_COUNT=0
MISMATCH_COUNT=0
NONE_COUNT=0
OBSERVED=0

# Get current tip so we know where to start observing from
CURRENT_HEX=$(get_best_number)
CURRENT=$(hex_to_dec "$CURRENT_HEX")
echo "[INFO] Current best block: #$CURRENT — waiting for next block..."

LAST_SEEN=$CURRENT
TARGET=$((CURRENT + BLOCKS))

while [ "$OBSERVED" -lt "$BLOCKS" ]; do
    CURRENT_HEX=$(get_best_number)
    CURRENT=$(hex_to_dec "$CURRENT_HEX")

    if [ "$CURRENT" -le "$LAST_SEEN" ]; then
        sleep "$POLL_INTERVAL"
        continue
    fi

    # Process all new blocks since last seen
    for ((BN = LAST_SEEN + 1; BN <= CURRENT && OBSERVED < BLOCKS; BN++)); do
        # 1. Get block hash at this height
        BLOCK_HASH=$(get_block_hash "$BN")
        if [ -z "$BLOCK_HASH" ] || [ "$BLOCK_HASH" = "null" ]; then
            echo "WARN: no hash for block #$BN"
            continue
        fi

        # 2. Get the actual block and extract digest
        BLOCK_JSON=$(get_block "$BLOCK_HASH")
        DIGEST_LOG=$(echo "$BLOCK_JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
logs = d.get('result', {}).get('block', {}).get('header', {}).get('digest', {}).get('logs', [])
# Find first PreRuntime log (starts with 0x06)
for log in logs:
    if log.startswith('0x06'):
        print(log)
        sys.exit(0)
print('NONE')
" 2>/dev/null)

        # 3. Extract actual author from digest
        if [ "$DIGEST_LOG" = "NONE" ] || [ -z "$DIGEST_LOG" ]; then
            ACTUAL_AUTHOR="NO_DIGEST"
        else
            RAW_AUTHOR=$(extract_digest_author "$DIGEST_LOG")
            ACTUAL_AUTHOR="$RAW_AUTHOR"
        fi

        # 4. Get expected author from runtime at parent block (best_hash at time of authoring)
        PARENT_HASH=$(echo "$BLOCK_JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('result', {}).get('block', {}).get('header', {}).get('parentHash', 'null'))
" 2>/dev/null)

        # Encode block_number as SCALE u32 little-endian
        BN_LE=$(python3 -c "import struct; print(struct.pack('<I', $BN).hex())")

        EXPECTED_RESP=$(rpc "{\"jsonrpc\":\"2.0\",\"method\":\"state_call\",\"params\":[\"DcfApi_get_expected_author\",\"0x$BN_LE\",\"$PARENT_HASH\"],\"id\":1}" 2>/dev/null)
        EXPECTED_RAW=$(echo "$EXPECTED_RESP" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(d.get('result', 'ERROR'))
" 2>/dev/null)
        EXPECTED_AUTHOR=$(decode_option_account_id "$EXPECTED_RAW")

        # 5. Compare
        if [ "$EXPECTED_AUTHOR" = "NONE" ]; then
            NONE_COUNT=$((NONE_COUNT + 1))
            MATCH="(no-expectation)"
        elif [ "$EXPECTED_AUTHOR" = "DECODE_ERROR:$EXPECTED_RAW" ]; then
            NONE_COUNT=$((NONE_COUNT + 1))
            MATCH="(rpc-error)"
        elif [ "$EXPECTED_AUTHOR" = "$ACTUAL_AUTHOR" ]; then
            MATCH="✓ MATCH"
            MATCH_COUNT=$((MATCH_COUNT + 1))
        else
            MATCH="✗ MISMATCH ← BUG"
            MISMATCH_COUNT=$((MISMATCH_COUNT + 1))
        fi

        printf "%-8s %-68s %-68s %s\n" "#$BN" "$EXPECTED_AUTHOR" "$ACTUAL_AUTHOR" "$MATCH"
        OBSERVED=$((OBSERVED + 1))
    done

    LAST_SEEN=$CURRENT
    sleep "$POLL_INTERVAL"
done

echo ""
echo "============================================================"
echo "  Summary after $OBSERVED blocks:"
echo "    Matches:       $MATCH_COUNT"
echo "    Mismatches:    $MISMATCH_COUNT  ← should be 0"
echo "    No-expectation: $NONE_COUNT     ← runtime returned None"
echo "============================================================"
if [ "$MISMATCH_COUNT" -gt 0 ]; then
    echo "  STATUS: FAIL — $MISMATCH_COUNT author mismatches detected. This is a structural problem."
    exit 1
else
    echo "  STATUS: PASS — All observed blocks had matching authors."
    exit 0
fi
