# Environment record

Recorded 19 September 2026 at the start of the Cerulea build.

## Host

| Item | Value |
|---|---|
| Operating system | Windows 11 Pro 10.0.26200 with the Linux subsystem, Ubuntu 24.04, version 2 |
| Linux kernel | 6.18.33.2-microsoft-standard-WSL2 |
| RAM | 32 GB physical. 26 GB allotted to the Linux subsystem. Reported inside Linux: 25 GB |
| Cores | 8 |
| Free disk | 489 GB free on the Windows C drive. 948G free on the Linux volume |
| Graphics | Intel(R) Arc(TM) 140V GPU (16GB) |

## Toolchain

| Tool | Version |
|---|---|
| rustc | 1.85.1 |
| cargo | 1.85.1 |
| clang | 18.1.3 |
| protoc | 3.21.12 |
| cmake | 3.28.3 |
| mold | 2.30.0 |
| sccache | 0.8.2 |
| node | v20.20.2 |
| pnpm | 9.15.9 |
| PostgreSQL | 16.15 |
| git | 2.43.0 |

## Layout

Every command runs inside Linux. The repositories live in the Windows
folder and are reached from Linux under /mnt/c. The Rust target directory
lives on the Linux volume at /root/cerulea-target, so every build artefact
is written and read at native speed. The source tree crosses the boundary
only on reads, which keeps the build within the 60 to 100 minute window.

The original Linux subsystem image on this machine failed to mount. A
fresh Ubuntu 24.04 image was installed beside it. Nothing in the build
depends on the original image.
