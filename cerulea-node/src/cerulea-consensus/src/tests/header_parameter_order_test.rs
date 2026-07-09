//! Tests for verifying correct Header::new parameter order and parent hash propagation

use sp_runtime::traits::{BlakeTwo256, Hash, Header as HeaderTrait};
use sp_runtime::generic::Header;

type TestHeader = Header<u32, BlakeTwo256>;

#[test]
fn test_header_new_parameter_order() {
    // Test that Header::new accepts parameters in the correct order:
    // (number, extrinsics_root, state_root, parent_hash, digest)
    
    let number = 42u32;
    let extrinsics_root = BlakeTwo256::hash(b"extrinsics");
    let state_root = BlakeTwo256::hash(b"state");
    let parent_hash = BlakeTwo256::hash(b"parent");
    let digest = Default::default();
    
    let header = TestHeader::new(
        number,
        extrinsics_root,
        state_root,
        parent_hash,
        digest,
    );
    
    // Verify all parameters are correctly set
    assert_eq!(*header.number(), number);
    assert_eq!(header.extrinsics_root(), &extrinsics_root);
    assert_eq!(header.state_root(), &state_root);
    assert_eq!(header.parent_hash(), &parent_hash);
}

#[test]
fn test_header_parameter_order_consistency() {
    // This test ensures that our parameter order matches Substrate's expectations
    // by creating headers with distinct values and verifying they're in the right positions
    
    let test_cases = vec![
        (1u32, "ext1", "state1", "parent1"),
        (2u32, "ext2", "state2", "parent2"),
        (3u32, "ext3", "state3", "parent3"),
    ];
    
    for (number, ext_data, state_data, parent_data) in test_cases {
        let extrinsics_root = BlakeTwo256::hash(ext_data.as_bytes());
        let state_root = BlakeTwo256::hash(state_data.as_bytes());
        let parent_hash = BlakeTwo256::hash(parent_data.as_bytes());
        
        let header = TestHeader::new(
            number,
            extrinsics_root,
            state_root,
            parent_hash,
            Default::default(),
        );
        
        // Verify each parameter is in the correct position
        assert_eq!(*header.number(), number, "Block number mismatch");
        assert_eq!(header.extrinsics_root(), &extrinsics_root, "Extrinsics root mismatch");
        assert_eq!(header.state_root(), &state_root, "State root mismatch");
        assert_eq!(header.parent_hash(), &parent_hash, "Parent hash mismatch");
    }
}