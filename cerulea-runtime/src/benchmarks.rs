// This file is part of the CBC Runtime.
// It defines the benchmarking configuration for various FRAME pallets used in the runtime.

// The macro `frame_benchmarking::define_benchmarks!` is used to register all the benchmarking
// test modules that will be executed during runtime performance measurement.
// Each entry represents a benchmarking target associated with a specific pallet/module.

frame_benchmarking::define_benchmarks!(
    // This line registers the benchmark module defined in `frame_benchmarking`, under `BaselineBench`,
    // which provides baseline benchmarks such as block execution time, extrinsics, etc.
    [frame_benchmarking, BaselineBench::<Runtime>]

    // Benchmarks for the `frame_system` pallet, which handles core blockchain functionality
    // such as block production, account management, and extrinsic validation.
    // `SystemBench` contains performance tests specific to the system operations.
    [frame_system, SystemBench::<Runtime>]

    // Benchmarks for the `pallet_balances`, which manages the account balance information.
    // It includes benchmarking of functions like transfers, setting balances, reserving funds, etc.
    [pallet_balances, Balances]

    // Benchmarks for `pallet_timestamp`, responsible for setting the on-chain timestamp.
    // Measures performance of time-setting and any time-dependent logic.
    [pallet_timestamp, Timestamp]

    // Benchmarks for the `pallet_sudo`, which allows privileged access to call any function.
    // These tests measure overhead and weight of administrative operations through sudo.
    [pallet_sudo, Sudo]

    // Benchmarks for the `pallet_cbc_dcf`, the main Dynamic Consensus Framework pallet.
    // It includes performance tests for validator management, consensus operations, governance,
    // epoch transitions, and all core DCF functionality.
    [pallet_cbc_dcf, DcfPallet]

    // Benchmarks for the `pallet_cbc_poi`, which handles Proof-of-Inference functionality.
    // It includes performance tests for inference submission, challenge mechanisms,
    // and inference validation operations.
    [pallet_cbc_poi, PalletCbcPoi]

    // Benchmarks for the `pallet_cbc_pos`, which handles Proof-of-Stake functionality.
    // It includes performance tests for validator registration, score submission,
    // slashing operations, and stake management.
    [pallet_cbc_pos, PalletCbcPos]
);
