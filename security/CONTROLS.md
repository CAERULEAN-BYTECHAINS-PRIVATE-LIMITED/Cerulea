# Security Control Register

Controls implemented, certification not held.

This register maps all twenty-five security controls (`P10-001` through `P10-025`) to ISO 27001, ISO 27017, ISO 27018, and SP 800-53 rev 5. The machine-readable register sits at `security/controls.json`.

Run `bash scripts/collect-chain-evidence.sh` to execute the chain proving commands, write `last-run.txt` under each chain control directory in `security/evidence/`, and generate the release software bill of materials.

| Control | Title | Scope | ISO 27001 | ISO 27017 | ISO 27018 | SP 800-53 rev 5 | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P10-001 | Equivocation detection and penalty | chain | A.8.16 | CLD.12.4.5 | A.10.1 | SI-4 | `security/evidence/P10-001` |
| P10-002 | Checkpoint anchoring against long-range attack | chain | A.8.16 | CLD.12.4.5 | A.10.1 | SI-4, SC-7 | `security/evidence/P10-002` |
| P10-003 | Peer diversity requirements against eclipse attack | chain | A.8.20 | CLD.13.1.4 | A.10.1 | SC-7 | `security/evidence/P10-003` |
| P10-004 | Bounded and repriced network-reachable message paths | chain | A.8.6 | CLD.12.1.3 | A.11.1 | SC-5 | `security/evidence/P10-004` |
| P10-005 | Blueprint static analysis before deployment | chain | A.8.25, A.8.26, A.8.28 | CLD.9.5.1 | A.2.1 | SA-11, SA-15, SI-10 | `security/evidence/P10-005` |
| P10-006 | The application security standard across every service | platform | A.8.26, A.8.27, A.8.8 | CLD.9.5.1, CLD.12.1.5 | A.10.1 | SA-8, SC-18, SI-10, RA-5 | `security/evidence/P10-006` |
| P10-007 | Multi-factor authentication on every account | platform | A.5.17, A.8.5 | CLD.9.5.2 | A.10.2 | IA-2, IA-5 | `security/evidence/P10-007` |
| P10-008 | Least-privilege role assignment throughout | chain | A.5.15, A.5.18, A.8.2 | CLD.9.5.2 | A.10.4 | AC-2, AC-3, AC-6 | `security/evidence/P10-008` |
| P10-009 | Hardware security module key custody | chain | A.8.24 | CLD.10.1.2 | A.10.5 | SC-12, SC-13 | `security/evidence/P10-009` |
| P10-010 | Encryption at rest across every store | ops | A.8.24, A.8.10 | CLD.10.1.2 | A.11.5 | SC-28 | `security/evidence/P10-010` |
| P10-011 | Mutual transport security between internal services | ops | A.8.20, A.8.21 | CLD.13.1.4 | A.11.6 | SC-8, SC-23 | `security/evidence/P10-011` |
| P10-012 | Field-level encryption for classified collections | chain | A.8.24, A.5.12 | CLD.10.1.2 | A.11.5 | SC-28, AC-3 | `security/evidence/P10-012` |
| P10-013 | Key rotation, escrow and destruction | chain | A.8.24 | CLD.10.1.2 | A.10.12 | SC-12, SC-17 | `security/evidence/P10-013` |
| P10-014 | Consent tracking per subject | chain | A.5.34 | CLD.6.3.1 | A.2.1, A.2.2 | PT-4, PT-5 | `security/evidence/P10-014` |
| P10-015 | Purpose limitation with denial and logging | chain | A.5.34, A.8.15 | CLD.6.3.1 | A.2.1 | PT-2, AU-2 | `security/evidence/P10-015` |
| P10-016 | Retention with automatic expiry action | chain | A.5.33, A.8.10 | CLD.8.1.5 | A.4.1 | SI-12, PT-2 | `security/evidence/P10-016` |
| P10-017 | Subject erasure preserving the audit commitment | chain | A.5.34, A.8.10 | CLD.8.1.5 | A.9.3 | SI-12, PT-6 | `security/evidence/P10-017` |
| P10-018 | Subject portability export | chain | A.5.34 | CLD.8.1.5 | A.1.1 | PT-6 | `security/evidence/P10-018` |
| P10-019 | Residency lock at the infrastructure layer | ops | A.5.31, A.5.34 | CLD.6.3.1, CLD.12.1.5 | A.11.12 | SA-9, PT-2 | `security/evidence/P10-019` |
| P10-020 | Network segmentation with default-deny egress | ops | A.8.20, A.8.22 | CLD.13.1.4 | A.11.6 | SC-7, AC-4 | `security/evidence/P10-020` |
| P10-021 | Reproducible builds | chain | A.8.31, A.8.32 | CLD.12.1.5 | A.10.1 | SA-15, CM-3 | `security/evidence/P10-021` |
| P10-022 | The software bill of materials per release | chain | A.5.21, A.8.30 | CLD.12.1.5 | A.10.1 | SR-4, SR-3 | `security/evidence/P10-022` |
| P10-023 | Artefact signing and verification | ops | A.5.21, A.8.31 | CLD.12.1.5 | A.10.1 | SR-4, SI-7 | `security/evidence/P10-023` |
| P10-024 | Immutable centralised logging with detection rules | ops | A.8.15, A.8.16 | CLD.12.4.5 | A.10.1 | AU-9, SI-4 | `security/evidence/P10-024` |
| P10-025 | Incident response procedures and the drill schedule | ops | A.5.24, A.5.25, A.5.26 | CLD.16.1.1 | A.9.1 | IR-4, IR-6, IR-3 | `security/evidence/P10-025` |
