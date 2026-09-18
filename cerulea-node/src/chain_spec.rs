use sc_service::ChainType;
use cerulea_runtime::WASM_BINARY;
use sc_chain_spec::Properties;
use serde::{Deserialize, Serialize};

// Chain constants
pub const TOKEN_SYMBOL: &str = "CERULEA";
pub const TOKEN_DECIMALS: u8 = 12;
pub const PROTOCOL_ID: &str = "cerulea";

pub type ChainSpec = sc_service::GenericChainSpec;

/// Chain metadata structure containing Cerulea-specific parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainMetadata {
    pub cerulea_epoch_length: u32,
    pub cerulea_validator_count: u32,
}

/// Returns chain properties with token information
pub fn chain_properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());
    properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
    properties.insert("ss58Format".into(), 42.into());
    properties
}

/// Returns chain metadata with Cerulea-specific parameters
#[allow(dead_code)]
pub fn chain_metadata() -> ChainMetadata {
    ChainMetadata {
        cerulea_epoch_length: 100,
        cerulea_validator_count: 10,
    }
}



pub fn development_chain_spec() -> Result<ChainSpec, String> {
    Ok(
        ChainSpec::builder(
            WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
            None,
        )
        .with_name("Cerulea Development Chain")
        .with_id("cerulea_dev")
        .with_chain_type(ChainType::Development)
        .with_protocol_id(PROTOCOL_ID)
        .with_properties(chain_properties())
        .with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
        .build()
    )
}

pub fn local_chain_spec() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
    
    // Created a minimal chain spec with properties and metadata
    let chain_spec = ChainSpec::builder(
        wasm_binary,
        None,
    )
    .with_name("Cerulea Local Testnet")
    .with_id("cerulea_local")
    .with_chain_type(ChainType::Local)
    .with_protocol_id(PROTOCOL_ID)
    .with_properties(chain_properties())
    .with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
    .build();
    
    Ok(chain_spec)
}

pub fn multi_validator_chain_spec() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
    
    let chain_spec = ChainSpec::builder(
        wasm_binary,
        None,
    )
    .with_name("Cerulea Multi-Validator Testnet")
    .with_id("cerulea_multi_validator")
    .with_chain_type(ChainType::Local)
    .with_protocol_id(PROTOCOL_ID)
    .with_properties(chain_properties())
    .with_genesis_config_preset_name("multi_validator")
    .build();
    
    Ok(chain_spec)
}

pub fn high_stake_chain_spec() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
    
    let chain_spec = ChainSpec::builder(
        wasm_binary,
        None,
    )
    .with_name("Cerulea High-Stake Testnet")
    .with_id("cerulea_high_stake")
    .with_chain_type(ChainType::Local)
    .with_protocol_id(PROTOCOL_ID)
    .with_properties(chain_properties())
    .with_genesis_config_preset_name("high_stake")
    .build();
    
    Ok(chain_spec)
}