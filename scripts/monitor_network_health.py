import subprocess
import json
import struct
import time
import sys
from collections import Counter

# Configuration for the 3-node network
NODES = {
    "Alice": "http://localhost:9944",
    "Bob":   "http://localhost:9945",
    "Charlie": "http://localhost:9946"
}

# Human-readable names mapping
MAPPING = {
    "0x88dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee": "ALICE",
    "0xd17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae69": "BOB",
    "0x439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f": "CHARLIE",
}

DOLLARS = 10**12

def rpc(url, method, params):
    cmd = ["curl", "-s", "-X", "POST", url, "-H", "Content-Type: application/json",
           "-d", json.dumps({"jsonrpc":"2.0", "method":method, "params":params, "id":1})]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        if r.returncode != 0 or not r.stdout.strip():
            return None
        return json.loads(r.stdout)
    except Exception:
        return None

def get_best_number(url):
    r = rpc(url, "chain_getHeader", [])
    if not r or "result" not in r: return None
    return int(r["result"]["number"], 16)

def get_block_hash(url, n):
    r = rpc(url, "chain_getBlockHash", [n])
    if not r or "result" not in r: return None
    return r["result"]

def get_block(url, h):
    r = rpc(url, "chain_getBlock", [h])
    if not r or "result" not in r: return None
    return r["result"]

def state_call_expected_author(url, block_num, at_hash):
    # SCALE encode u32 little-endian
    encoded = struct.pack("<I", block_num).hex()
    r = rpc(url, "state_call", ["DcfApi_get_expected_author", f"0x{encoded}", at_hash])
    if not r or "result" not in r: return None, "ERR"
    res = r["result"]
    if not res or res == "0x": return None, "NONE"
    # Option<AccountId>: 0x01 + 32-byte public key
    if res.startswith("0x01") and len(res) >= 66:
        return "0x" + res[4:68], "OK"
    return None, "NONE"

def get_validator_metrics(url):
    """Fetch scores, participation, and stake for all active validators"""
    metrics = {}
    r = rpc(url, "state_call", ["DcfApi_get_active_validators", "0x"])
    if not r or "result" not in r: return {}
    
    # result is a hex-encoded Vec<AccountId>
    hex_data = r["result"][2:]
    # Each AccountId is 32 bytes (64 hex chars). The first byte is the length (simplified)
    # Actually, for 3 accounts, 0x0c (3 accounts * 4?) - Scale encoding for Vec is compact-encoded len.
    # 3 in compact is 0x0c (3 << 2).
    # We'll just split by 64 chars after the length byte.
    idx = 2
    while idx < len(hex_data):
        acc_hex = "0x" + hex_data[idx:idx+64]
        if len(acc_hex) < 66: break
        
        # Get Score
        r_score = rpc(url, "state_call", ["DcfApi_get_validator_scores", "0x"])
        score = 0
        if r_score and "result" in r_score:
            # Parse Vec<(AccountId, u64)> - simplified search
            if hex_data[idx:idx+64] in r_score["result"]:
                score_idx = r_score["result"].find(hex_data[idx:idx+64]) + 64
                # u64 is 8 bytes = 16 hex chars, little-endian
                score_hex = r_score["result"][score_idx:score_idx+16]
                if len(score_hex) == 16:
                    score = struct.unpack("<Q", bytes.fromhex(score_hex))[0]

        # Get Participation (authored, missed)
        r_part = rpc(url, "state_call", ["DcfApi_get_validator_participation", acc_hex])
        authored, missed = 0, 0
        if r_part and "result" in r_part and r_part["result"] != "0x":
            res = bytes.fromhex(r_part["result"][2:])
            if len(res) >= 8:
                authored = struct.unpack("<I", res[0:4])[0]
                missed = struct.unpack("<I", res[4:8])[0]

        # Get Stake
        r_stake = rpc(url, "state_call", ["DcfApi_get_validator_stake", acc_hex])
        stake = 0
        if r_stake and "result" in r_stake and r_stake["result"] != "0x":
            # u128 is 16 bytes = 32 hex chars, little-endian
            res = bytes.fromhex(r_stake["result"][2:])
            if len(res) >= 16:
                stake = struct.unpack("<QQ", res[0:16])
                stake = stake[0] + (stake[1] << 64)

        metrics[acc_hex] = {
            "name": MAPPING.get(acc_hex, acc_hex[:10]),
            "score": score,
            "authored": authored,
            "missed": missed,
            "stake": stake / DOLLARS
        }
        idx += 64
    return metrics

def extract_author(block):
    """Extract block author from PreRuntime digest"""
    header = block.get("block", {}).get("header", {})
    logs = header.get("digest", {}).get("logs", [])
    for log in logs:
        if log.startswith("0x0663626364"): # 06 (PreRuntime) + cbcd (63626364)
            # Layout: 06 (1B) + 63626364 (4B) + 80 (1B compact len) + 32B PubKey
            # Indices: 0x(0,1) 06(2,3) cbcd(4,11) 80(12,13) Pub(14,77)
            if len(log) >= 78:
                return "0x" + log[14:78].lower()
    return None

def check_sync_status():
    print(f"{'Node':<10} | {'Best Block':<12} | {'Status':<10}")
    print("-" * 38)
    heights = {}
    for name, url in NODES.items():
        h = get_best_number(url)
        heights[name] = h
        status = "ONLINE" if h is not None else "OFFLINE"
        print(f"{name:<10} | {str(h):<12} | {status:<10}")
    return heights

def monitor_consensus(rounds=20):
    print("\n[INFO] Starting Consensus Health Monitor (observing {} blocks)...".format(rounds))
    print(f"{'Block':>6} | {'Author (Expected)':<15} | {'Author (Actual)':<15} | {'Match?':<8} | {'Latency'}")
    print("-" * 80)
    
    seen_blocks = set()
    matches = 0
    mismatches = 0
    author_counts = Counter()
    
    # Use Alice as the primary source of truth for the chain
    primary_url = NODES["Alice"]
    
    start_block = get_best_number(primary_url)
    if start_block is None:
        print("[ERROR] Alice is offline. Cannot proceed.")
        return

    # Initial metrics for baseline
    baseline_metrics = get_validator_metrics(primary_url)

    while len(seen_blocks) < rounds:
        best = get_best_number(primary_url)
        if best is None:
            time.sleep(1)
            continue
            
        for bn in range(start_block, best + 1):
            if bn in seen_blocks or len(seen_blocks) >= rounds:
                continue
            
            bh = get_block_hash(primary_url, bn)
            blk = get_block(primary_url, bh)
            if not blk: continue
            
            # Identify Author
            actual = extract_author(blk)
            parent = blk["block"]["header"]["parentHash"]
            expected, status = state_call_expected_author(primary_url, bn, parent)
            
            # Print block info
            act_name = MAPPING.get(actual, actual[:10] if actual else "NONE")
            exp_name = MAPPING.get(expected, expected[:10] if expected else "NONE")
            
            match = "✓ YES" if actual == expected else "✗ NO"
            if actual == expected: matches += 1
            else: mismatches += 1
            
            if actual: author_counts[act_name] += 1
            
            print(f"{bn:>6} | {exp_name:<15} | {act_name:<15} | {match:<8} | OK")
            
            # Fetch and print detailed metrics every block
            metrics = get_validator_metrics(primary_url)
            print("-" * 80)
            print(f"{'Validator':<10} | {'Score':<6} | {'Authored':<9} | {'Missed':<6} | {'Stake (CBC )':<12} | {'G/L'}")
            for acc, m in metrics.items():
                base = baseline_metrics.get(acc, {"stake": m["stake"]})
                gain = m["stake"] - base["stake"]
                gain_str = f"{gain:+.4f}" if gain != 0 else "0.0000"
                print(f"{m['name']:<10} | {m['score']:<6} | {m['authored']:<9} | {m['missed']:<6} | {m['stake']:<12.4f} | {gain_str}")
            print("-" * 80)
            
            seen_blocks.add(bn)
            sys.stdout.flush()
            
        time.sleep(1)

    print("\n" + "="*50)
    print("CONSENSUS HEALTH REPORT")
    print("="*50)
    print(f"Total Blocks Observed:  {len(seen_blocks)}")
    print(f"Total Matches:          {matches}")
    print(f"Total Mismatches:       {mismatches}")
    print("-" * 30)
    print("Block Production Participation:")
    for name, count in author_counts.items():
        percentage = (count / len(seen_blocks)) * 100
        print(f"  {name:<10}: {count:>3} blocks ({percentage:>5.1f}%)")
    
    if len(author_counts) < 3:
        print("\n[WARNING] Only {}/3 validators are participating in block production!".format(len(author_counts)))
        if "CHARLIE" not in author_counts:
            print("[CRITICAL] CHARLIE is NOT producing blocks.")
    else:
        print("\n[SUCCESS] All 3 validators (Alice, Bob, Charlie) are participating.")
    
    if mismatches > 0:
        print("[FAIL] Consensus mismatch detected! Check PreRuntime digests.")
    elif matches == len(seen_blocks):
        print("[PASS] Deterministic Round-Robin rotation is working perfectly.")

if __name__ == "__main__":
    print("CBC Network Health Monitor v1.0")
    print("===============================")
    syncs = check_sync_status()
    if all(h is not None for h in syncs.values()):
        monitor_consensus(30)
    else:
        print("\n[ERROR] One or more nodes are OFFLINE. Please check startup scripts.")
