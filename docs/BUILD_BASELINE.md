# Build baseline

Recorded 19 September 2026 on the rebuild/foundation branch after the
P0 cleanup and the identifier renames, before any change to logic.

## Release build

| Measure | Value |
|---|---|
| Command | `CARGO_BUILD_JOBS=4 cargo build --release` inside Linux |
| Target directory | /root/cerulea-target on the Linux volume |
| Cold duration | 14 minutes 38 seconds to the runtime crate, then 3 minutes 33 seconds after the runtime builder fix. Roughly 19 minutes in total |
| Binary | /root/cerulea-target/release/cerulea-node, 82,222,744 bytes |
| Binary version | cerulea-node 0.1.0 |
| Target directory size | 4.6 GB |
| Compilation cache size | 977 MB |

The cold build finished well inside the 60 to 100 minute window the
specification allows, since the Linux volume carries the target directory
and sccache carries the object cache.

## Block production

`cerulea-node --dev --tmp --enable-cerulea-extensions` ran for 90 seconds.
The node imported 15 blocks in that window, one every six seconds, with no
error and no panic in the log.

## Runtime builder note

The runtime builder searches upward from the target directory for the
workspace lock file. With the target directory outside the workspace the
search failed and the builder resolved every crate afresh, which pulled a
crate needing a newer compiler than the pinned 1.85. `.cargo/config.toml`
now carries `WASM_BUILD_WORKSPACE_HINT` pointing at the workspace root.
