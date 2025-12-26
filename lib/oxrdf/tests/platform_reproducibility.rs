//! Platform reproducibility tests
//!
//! This test suite validates that BlankNode serialization is platform-independent
//! and reproducible across different architectures (big-endian vs little-endian).

use oxrdf::BlankNode;
use std::collections::HashSet;

#[test]
fn test_blank_node_default_uniqueness() {
    // Verify that BlankNode::default() creates unique IDs
    let mut ids = HashSet::new();

    for _ in 0..1000 {
        let bn = BlankNode::default();
        let id = bn.as_str().to_string();
        assert!(
            ids.insert(id.clone()),
            "BlankNode::default() created duplicate ID: {}",
            id
        );
    }

    println!("✓ Generated 1000 unique blank node IDs");
}

#[test]
fn test_blank_node_from_unique_id_consistency() {
    // Test that creating a BlankNode from the same numerical ID
    // always produces the same string representation
    let test_ids = vec![
        0u128,
        1u128,
        42u128,
        0x100au128,
        0xdeadbeefu128,
        0x7777_6666_5555_4444_3333_2222_1111_0000u128,
        u128::MAX,
    ];

    for &id in &test_ids {
        let results: Vec<String> = (0..10)
            .map(|_| BlankNode::new_from_unique_id(id).as_str().to_string())
            .collect();

        let first = &results[0];
        for (i, result) in results.iter().enumerate().skip(1) {
            assert_eq!(
                first, result,
                "BlankNode::new_from_unique_id({}) produced inconsistent string on run {}: expected '{}', got '{}'",
                id, i, first, result
            );
        }
    }

    println!("✓ BlankNode string representation is consistent for same numerical ID");
}

#[test]
fn test_blank_node_new_from_string_consistency() {
    // Test that creating a BlankNode from the same string ID
    // always produces the same internal representation
    let test_ids = vec!["a", "abc", "test123", "100a", "deadbeef"];

    for id in test_ids {
        let nodes: Vec<BlankNode> = (0..10)
            .map(|_| BlankNode::new(id).unwrap())
            .collect();

        let first = &nodes[0];
        for (i, node) in nodes.iter().enumerate().skip(1) {
            assert_eq!(
                first, node,
                "BlankNode::new('{}') produced inconsistent node on run {}",
                id, i
            );
        }
    }

    println!("✓ BlankNode creation from string is consistent");
}

#[test]
fn test_blank_node_round_trip_numerical_id() {
    // Test that numerical ID can be round-tripped
    let test_ids = vec![
        0u128,
        1u128,
        42u128,
        0x100au128,
        0xdeadbeefu128,
        0x7777_6666_5555_4444_3333_2222_1111_0000u128,
    ];

    for &id in &test_ids {
        let bn = BlankNode::new_from_unique_id(id);
        let extracted_id = bn.as_ref().unique_id();

        assert_eq!(
            Some(id),
            extracted_id,
            "Round-trip failed for ID {}: got {:?}",
            id,
            extracted_id
        );
    }

    println!("✓ Numerical ID round-trip successful");
}

#[test]
fn test_blank_node_equality_consistency() {
    // Test that equality is consistent across multiple checks
    let bn1 = BlankNode::new_from_unique_id(12345);
    let bn2 = BlankNode::new_from_unique_id(12345);
    let bn3 = BlankNode::new_from_unique_id(54321);

    // Same ID should always be equal
    for _ in 0..100 {
        assert_eq!(bn1, bn2, "Same numerical ID should always be equal");
        assert_ne!(bn1, bn3, "Different numerical IDs should never be equal");
    }

    println!("✓ BlankNode equality is consistent");
}

#[test]
fn test_blank_node_hash_consistency() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Test that hash values are consistent
    let bn = BlankNode::new_from_unique_id(0xdeadbeef);

    let hashes: Vec<u64> = (0..100)
        .map(|_| {
            let mut hasher = DefaultHasher::new();
            bn.hash(&mut hasher);
            hasher.finish()
        })
        .collect();

    let first_hash = hashes[0];
    for (i, &hash) in hashes.iter().enumerate().skip(1) {
        assert_eq!(
            first_hash, hash,
            "Hash value changed on iteration {}: expected {}, got {}",
            i, first_hash, hash
        );
    }

    println!("✓ BlankNode hash values are consistent");
}

#[test]
fn test_blank_node_internal_representation() {
    // This test documents the current behavior with to_ne_bytes()
    // WARNING: This is platform-specific and may fail on big-endian systems!

    let id = 0x0102030405060708090a0b0c0d0e0f10u128;
    let bn = BlankNode::new_from_unique_id(id);

    // On little-endian (x86_64, most ARM):
    // Bytes should be: 10 0f 0e 0d 0c 0b 0a 09 08 07 06 05 04 03 02 01
    // On big-endian (some PowerPC, MIPS):
    // Bytes should be: 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f 10

    // We can detect endianness:
    let is_little_endian = cfg!(target_endian = "little");
    let is_big_endian = cfg!(target_endian = "big");

    println!("Platform endianness: little={}, big={}", is_little_endian, is_big_endian);

    // Extract the ID back
    let extracted_id = bn.as_ref().unique_id().unwrap();

    // This SHOULD work on both platforms if to_ne_bytes/from_ne_bytes are paired
    assert_eq!(
        id, extracted_id,
        "ID extraction failed: expected {:x}, got {:x}",
        id, extracted_id
    );

    println!("✓ Internal representation is self-consistent on this platform");
    println!("⚠ WARNING: This does NOT guarantee cross-platform compatibility!");
}

#[test]
fn test_cross_platform_portability_warning() {
    // This test documents the portability issue with to_ne_bytes()

    let id = 0xdeadbeefu128;
    let bn = BlankNode::new_from_unique_id(id);

    // Get the string representation - this SHOULD be portable
    let str_repr = bn.as_str();

    // Recreate from string
    let bn2 = BlankNode::new(str_repr).unwrap();

    // These should be equal
    assert_eq!(bn, bn2, "String round-trip should preserve equality");

    println!("✓ String representation provides portable serialization");
    println!("⚠ CRITICAL: Do NOT serialize the internal [u8; 16] representation!");
    println!("⚠ CRITICAL: to_ne_bytes() causes platform-specific binary format!");
    println!("⚠ RECOMMENDATION: Use to_le_bytes() for portable storage");
}

#[test]
fn test_blank_node_serialization_recommendation() {
    // Demonstrate the CORRECT way to serialize blank nodes portably

    let id = 0x123456789abcdef0u128;
    let bn = BlankNode::new_from_unique_id(id);

    // CORRECT: Use string representation for portability
    let portable_repr = bn.as_str().to_string();

    // INCORRECT (current implementation): Using to_ne_bytes()
    // This creates platform-specific bytes that can't be shared between
    // little-endian and big-endian systems

    // Verify string representation is hex of the ID
    let expected = format!("{:x}", id);
    assert_eq!(portable_repr, expected);

    println!("✓ Portable serialization: use as_str() = '{}'", portable_repr);
    println!("✗ Non-portable: internal [u8; 16] with to_ne_bytes()");
}

#[test]
fn test_demonstrate_to_le_bytes_fix() {
    // This demonstrates what the fix should look like

    let id = 0x0102030405060708090a0b0c0d0e0f10u128;

    // Current (platform-specific):
    let ne_bytes = id.to_ne_bytes();

    // Recommended (portable):
    let le_bytes = id.to_le_bytes();

    // On little-endian systems, these are the same
    // On big-endian systems, they are reversed
    if cfg!(target_endian = "little") {
        assert_eq!(ne_bytes, le_bytes, "On little-endian, ne_bytes == le_bytes");
        println!("✓ Platform: little-endian (x86_64, most ARM)");
    } else if cfg!(target_endian = "big") {
        assert_ne!(ne_bytes, le_bytes, "On big-endian, ne_bytes != le_bytes");
        println!("✓ Platform: big-endian (some PowerPC, MIPS, older ARM)");
    }

    // to_le_bytes() always produces the same byte order:
    // [0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09,
    //  0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]

    // This can be deserialized on ANY platform using from_le_bytes()
    let recovered = u128::from_le_bytes(le_bytes);
    assert_eq!(id, recovered);

    println!("✓ to_le_bytes() + from_le_bytes() is portable across platforms");
    println!("RECOMMENDATION: Replace to_ne_bytes() with to_le_bytes() in blank_node.rs");
}

#[test]
fn test_multiple_blank_nodes_order_preservation() {
    // Test that multiple blank nodes maintain their relationships

    let ids = vec![100u128, 200u128, 300u128];
    let nodes: Vec<BlankNode> = ids.iter().map(|&id| BlankNode::new_from_unique_id(id)).collect();

    // Extract IDs back
    let extracted: Vec<u128> = nodes
        .iter()
        .map(|bn| bn.as_ref().unique_id().unwrap())
        .collect();

    assert_eq!(ids, extracted, "ID preservation failed");

    println!("✓ Multiple blank nodes preserve their IDs correctly");
}
