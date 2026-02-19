# CBC Chain Network Startup Guide

This guide explains how to start the CBC Chain network in various configurations using persistent data storage, moving beyond the simple `--dev` mode.

## Core Concepts for Persistence

By default, Substrate-based nodes like `cbc-node` use a temporary database when run with `--dev`. To ensure your data persists across restarts, you must manage your **Base Path**.

*   `--base-path <PATH>`: Specifies the directory where the chain database, networking keys, and configurations are stored.
*   `--chain <CHAIN_SPEC>`: Specifies which chain configuration to use (e.g., `local`, `multi_validator`).
*   `--alice`, `--bob`, etc.: Pre-defined development keys used for block production in test environments.

---

## Mandatory Prerequisite: Network Key Generation

Unlike standard Substrate nodes, the CBC Chain requires manual generation of networking keys before the first startup when using a persistent base path. If this file is missing, the node will fail with a `NetworkKeyNotFound` error.

### 1. Create the Network Directory
You must create the nested directory structure inside your base path first:
```bash
# For Alice (local chain)
mkdir -p /tmp/cbc-alice/chains/cbc_local/network
```

### 2. Generate the Secret Key
Run the following command to generate the node's permanent identity:
```bash
./target/release/cbc-node key generate-node-key --file /tmp/cbc-alice/chains/cbc_local/network/secret_ed25519
```
**Important:** This command will output your `Local node identity` (e.g., `12D3KooW...`). Record this ID; it is required if you want other nodes to use this one as a bootnode.

---

## 1. Single-Node Setup (Persistent)

Use this for local development where you want to keep your account balances and transaction history after stopping the node.

### Start the Node
```bash
./target/release/cbc-node \
  --base-path /tmp/cbc-alice \
  --chain local \
  --alice \
  --port 30333 \
  --rpc-port 9944 \
  --validator \
  --name "Alice-Persistent"
```

*   **Alice** is the first validator in the `local` chain spec.
*   The data will be stored in `/tmp/cbc-alice`. Change this to a permanent path like `~/.cbc-data/alice` for true persistence.

### Restarting
Simply run the same command again. The node will pick up from the last block stored in the `--base-path`.

### To Clear Data
If you want to start fresh:
```bash
./target/release/cbc-node purge-chain --base-path /tmp/cbc-alice --chain local -y
```

---

## 2. Multi-Node Setup (Local Testnet)

To run a real network simulation with multiple validators communicating over P2P.

### Node 1: Alice (Bootnode)
Alice will act as the entry point for other nodes.

```bash
./target/release/cbc-node \
  --base-path /tmp/cbc-alice \
  --chain local \
  --alice \
  --port 30333 \
  --rpc-port 9944 \
  --validator \
  --name "Alice-Node"
```

**Find Alice's Node ID:**
Look for a log line like:
`Local node identity is: 12D3KooWxxxx...`

### Node 2: Bob (Joining Alice)
Bob needs to know Alice's address to connect.

```bash
./target/release/cbc-node \
  --base-path /tmp/cbc-bob \
  --chain local \
  --bob \
  --port 30334 \
  --rpc-port 9945 \
  --validator \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/ALICE_NODE_ID \
  --name "Bob-Node"
```
*(Replace `ALICE_NODE_ID` with the actual ID from Alice's logs)*

---

## 3. High-Stake/Multi-Validator Modes

CBC Chain comes with specialized chain specs located in `cbc-node/src/chain_spec.rs`:

*   **`local`**: Standard local testnet (Alice & Bob).
*   **`multi_validator`**: Setup with 4+ validators.
*   **`high_stake`**: Configuration for stress testing staking mechanisms.

### Example: Running Multi-Validator Mode
```bash
./target/release/cbc-node \
  --chain multi_validator \
  --base-path /tmp/cbc-data \
  --validator \
  --alice
```

---

## 4. Production-Ready Flags

When moving towards a production-like environment, consider these additional flags:

| Flag | Description |
| :--- | :--- |
| `--rpc-external` | Listen on all interfaces (careful: secure with firewall). |
| `--rpc-methods Safe` | Disable dangerous RPC calls (default for external). |
| `--prometheus-external` | Expose metrics for Grafana/Prometheus. |
| `--enable-cbc-extensions` | Enables CBC-specific RPC endpoints. |
| `--log-file <path>` | Redirect logs to a file for rotation/analysis. |

### Complete Persistent Multi-Node Command (Alice)
```bash
./target/release/cbc-node \
  --chain local \
  --base-path ./data/alice \
  --alice \
  --validator \
  --enable-cbc-extensions \
  --rpc-cors all \
  --name "CBC-Alpha-1"
```
