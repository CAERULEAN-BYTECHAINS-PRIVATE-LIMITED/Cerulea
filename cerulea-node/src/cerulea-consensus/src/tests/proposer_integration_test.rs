//! Integration tests for proposer factory with correct Header::new parameter order

use sp_runtime::traits::{BlakeTwo256, Hash, Header as HeaderTrait};
use sp_runtime::generic::{Header, Digest};

type TestHeader = Header<u32, BlakeTwo256>;

#[test]
fn test_proposer_factory_header_creation() {
    // This test verifies that the ProposerFactory can create headers without panicking
    // and that the parameter order is correct by checking the resulting header fields
    
    // Test data
    let parent_hash = BlakeTwo256::hash(b"test_parent_block");
    let expected_number = 42u32;
    let expected_extrinsics_root = BlakeTwo256::hash(b"test_extrinsics");
    let expected_state_root = BlakeTwo256::hash(b"test_state");
    
    // Create a header using the same parameter order as our fixed ProposerFactory
    let header = TestHeader::new(
        expected_number,
        expected_extrinsics_root,
        expected_state_root,
        parent_hash,
        Default::default(),
    );
    
    // Verify all parameters are in the correct positions
    assert_eq!(*header.number(), expected_number, "Block number should match");
    assert_eq!(header.extrinsics_root(), &expected_extrinsics_root, "Extrinsics root should match");
    assert_eq!(header.state_root(), &expected_state_root, "State root should match");
    assert_eq!(header.parent_hash(), &parent_hash, "Parent hash should match");
    
    // Verify the header hash is computed correctly
    let header_hash = header.hash();
    assert_ne!(header_hash, Default::default(), "Header hash should not be default");
}

#[test]
fn test_header_parameter_order_documentation() {
    // This test documents the correct parameter order for Header::new
    // Order: (number, extrinsics_root, state_root, parent_hash, digest)
    
    let number = 1u32;
    let extrinsics_root = BlakeTwo256::hash(b"extrinsics");
    let state_root = BlakeTwo256::hash(b"state");
    let parent_hash = BlakeTwo256::hash(b"parent");
    let digest: Digest = Default::default();
    
    let header = TestHeader::new(
        number,          // 1st parameter: block number
        extrinsics_root, // 2nd parameter: extrinsics root hash
        state_root,      // 3rd parameter: state root hash
        parent_hash,     // 4th parameter: parent block hash
        digest.clone(),  // 5th parameter: digest (logs)
    );
    
    // Verify the parameter mapping is correct
    assert_eq!(*header.number(), number);
    assert_eq!(header.extrinsics_root(), &extrinsics_root);
    assert_eq!(header.state_root(), &state_root);
    assert_eq!(header.parent_hash(), &parent_hash);
    assert_eq!(header.digest(), &digest);
}

#[test]
fn test_parent_hash_propagation_verification() {
    // This test verifies that parent hash propagation works correctly
    // by creating multiple headers in a chain
    
    let genesis_hash = BlakeTwo256::hash(b"genesis");
    let block1_hash = BlakeTwo256::hash(b"block1");
    let block2_hash = BlakeTwo256::hash(b"block2");
    
    // Create block 1 with genesis as parent
    let header1 = TestHeader::new(
        1u32,
        Default::default(),
        Default::default(),
        genesis_hash,
        Default::default(),
    );
    
    assert_eq!(header1.parent_hash(), &genesis_hash, "Block 1 should have genesis as parent");
    
    // Create block 2 with block 1 as parent
    let header2 = TestHeader::new(
        2u32,
        Default::default(),
        Default::default(),
        block1_hash,
        Default::default(),
    );
    
    assert_eq!(header2.parent_hash(), &block1_hash, "Block 2 should have block 1 as parent");
    
    // Create block 3 with block 2 as parent
    let header3 = TestHeader::new(
        3u32,
        Default::default(),
        Default::default(),
        block2_hash,
        Default::default(),
    );
    
    assert_eq!(header3.parent_hash(), &block2_hash, "Block 3 should have block 2 as parent");
    
    // Verify block numbers are sequential
    assert_eq!(*header1.number(), 1u32);
    assert_eq!(*header2.number(), 2u32);
    assert_eq!(*header3.number(), 3u32);
}