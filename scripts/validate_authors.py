import subprocess
import json
import struct
import time
import sys

RPC = "http://localhost:9944"

def rpc(method, params):
    cmd = ["curl", "-s", "-X", "POST", RPC, "-H", "Content-Type: application/json",
           "-d", json.dumps({"jsonrpc":"2.0","method":method,"params":params,"id":1})]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        if r.returncode != 0 or not r.stdout.strip():
            return None
        return json.loads(r.stdout)
    except Exception as e:
        return None

def get_best_number():
    r = rpc("chain_getHeader", [])
    if not r or "result" not in r: return None
    return int(r["result"]["number"], 16)

def get_block_hash(n):
    r = rpc("chain_getBlockHash", [n])
    if not r or "result" not in r: return None
    return r["result"]

def get_block(h):
    r = rpc("chain_getBlock", [h])
    if not r or "result" not in r: return None
    return r["result"]

def state_call_expected_author(block_num, at_hash):
    # SCALE encode u32 little-endian
    encoded = struct.pack("<I", block_num).hex()
    r = rpc("state_call", ["DcfApi_get_expected_author", f"0x{encoded}", at_hash])
    if not r or "result" not in r:
        return None, "API_ERROR"
    result_hex = r["result"]
    if not result_hex or result_hex in ("null", "0x"):
        return None, "NULL_RESPONSE"
    stripped = result_hex[2:] if result_hex.startswith("0x") else result_hex
    if len(stripped) < 2:
        return None, f"SHORT:{stripped}"
    option_byte = stripped[:2]
    if option_byte == "00":
        return None, "None"
    elif option_byte == "01" and len(stripped) >= 66:
        return "0x" + stripped[2:66], "Some"
    return None, f"UNKNOWN:{stripped[:10]}"

def extract_digest_author(logs):
    """Extract author from PreRuntime digest log (0x06 prefix = PreRuntime)"""
    for log in logs:
        if not log.startswith("0x"):
            continue
        stripped = log[2:]
        if len(stripped) < 2:
            continue
        log_type = stripped[:2]
        if log_type == "06":  # PreRuntime
            # layout: type(1B) + engine_id(4B) + SCALE-compact-len + data
            # "cbcd" = 0x63626364
            # After type(1) + engine(4) = 5B = 10 hex chars
            # Then SCALE compact len: len 32 = 0x80 (1B) = 2 hex
            if len(stripped) >= 12 + 64:
                engine = stripped[2:10]
                # Skip compact length byte (1 byte = 2 hex chars) at offset 10
                author_hex = stripped[12:76]
                return f"0x{author_hex}", f"engine=0x{engine}"
    return None, "NO_PRERUNTIME_DIGEST"

print(f"{'Block':>8} | {'Expected Author (runtime)':>66} | {'Actual Author (digest)':>66} | Result")
print("-" * 220)

TOTAL = 30
observed = 0
seen = set()
mismatches = 0
matches = 0
no_expected = 0
no_digest = 0

current = get_best_number()
if current is None:
    print("[ERROR] Could not connect to node or get best number")
    sys.exit(1)
    
print(f"[INFO] Starting at block #{current}, observing next {TOTAL} blocks...")

while observed < TOTAL:
    best = get_best_number()
    if best is None:
        time.sleep(1)
        continue
    for bn in range(current, best + 1):
        if bn in seen or observed >= TOTAL:
            continue
        seen.add(bn)
        bh = get_block_hash(bn)
        if not bh:
            continue
        blk = get_block(bh)
        if not blk:
            continue
        
        header = blk.get("block", {}).get("header", {})
        logs = header.get("digest", {}).get("logs", [])
        parent_hash = header.get("parentHash", bh)
        
        # Get expected author from runtime at parent (the state when block was authored)
        exp_author, exp_status = state_call_expected_author(bn, parent_hash)
        
        # Get actual author from digest
        act_author, act_status = extract_digest_author(logs)
        
        # Compare
        if exp_status == "None" or exp_author is None:
            result = f"⚠ no-expected ({exp_status})"
            no_expected += 1
        elif act_author is None:
            result = f"⚠ no-digest ({act_status})"
            no_digest += 1
        elif exp_author.lower() == act_author.lower():
            result = "✓ MATCH"
            matches += 1
        else:
            result = "✗ MISMATCH ← PROBLEM"
            mismatches += 1
        
        exp_display = exp_author or f"({exp_status})"
        act_display = act_author or f"({act_status})"
        print(f"#{bn:>7} | {exp_display:>66} | {act_display:>66} | {result}")
        sys.stdout.flush()
        
        observed += 1
    current = best + 1
    if observed < TOTAL:
        time.sleep(1)

print("\n" + "=" * 100)
print(f"Summary after {observed} blocks:")
print(f"  Matches:            {matches}")
print(f"  Mismatches:         {mismatches}  ← must be 0")
print(f"  No expected author: {no_expected}")
print(f"  No digest:          {no_digest}")
print("=" * 100)
if mismatches > 0:
    print("STATUS: FAIL - structural author mismatch detected")
    sys.exit(1)
elif no_digest == observed:
    print("STATUS: WARN - No digest authors in any block (blocks have no PreRuntime digest)")
elif matches > 0:
    print("STATUS: PASS - All matched blocks have consistent authors")
else:
    print("STATUS: INCOMPLETE - insufficient data for full validation")
