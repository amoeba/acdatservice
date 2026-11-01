use acdatservice::{
    appearance::{Appearance, AppearanceError},
    landblock_instances::{self as acli, LandblockInstancesError as Error},
};

// Produced independently by ac-noclip's encodeAcli/encodeAppearance. The exact
// same three-record synthetic input is in examples/acli_appearance.rs.
fn golden() -> Vec<u8> {
    let hex = include_str!("fixtures/acli-appearance.hex").trim();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn typescript_golden_roundtrips_present_absent_and_empty_appearances() {
    let bytes = golden();
    let value = acli::decode(&bytes).unwrap();
    assert_eq!(bytes.len(), 262);
    assert_eq!(value.landblock, 0x0125);
    assert_eq!(value.instances.len(), 3);
    assert_eq!(value.feature_bits(), acli::ACLI_FEATURE_APPEARANCE);
    let first = &value.instances[0];
    assert_eq!(first.guid, 0x8000_0001);
    assert_eq!(first.texture_overrides.len(), 1);
    assert_eq!(first.texture_overrides[0].texture_id, 0x0500_0003);
    let appearance = first.appearance.as_ref().unwrap();
    assert_eq!(appearance.palette_base_id, 0x0400_0001);
    assert_eq!(appearance.parts[0].gfx_obj_id, 0x0100_1234);
    assert_eq!(appearance.textures[0].old_texture_id, 0x0500_0001);
    assert_eq!(appearance.textures[0].new_texture_id, 0x0500_0002);
    assert_eq!(appearance.palettes[0].offset, 2); // blocks of eight, not colors
    assert_eq!(appearance.palettes[0].length, 4);
    assert_eq!(value.instances[1].appearance, None);
    assert_eq!(value.instances[2].appearance, Some(Appearance::default()));
    assert_eq!(value.encoded_len(), bytes.len());
    assert_eq!(acli::encode(&value), bytes);
}

#[test]
fn feature_free_layout_is_exactly_sixty_bytes_and_keeps_guid() {
    let mut value = acli::decode(&golden()).unwrap();
    for instance in &mut value.instances {
        instance.appearance = None;
    }
    let bytes = acli::encode(&value);
    assert_eq!(acli::RECORD_BASE, 60);
    assert_eq!(&bytes[6..8], &[0, 0]);
    assert_eq!(bytes.len(), 16 + 3 * 60 + 6);
    assert_eq!(acli::decode(&bytes).unwrap(), value);
    // The Rust codec intentionally only accepts the current base, not a guessed
    // legacy 56-byte layout. Routing raw R2 bytes remains independent of decoding.
    value.instances.truncate(1);
    value.instances[0].texture_overrides.clear();
    let mut legacy = acli::encode(&value);
    legacy.truncate(16 + 56);
    assert!(acli::decode(&legacy).is_err());
}

#[test]
fn rejects_every_truncation_without_panicking_or_legacy_fallback() {
    let bytes = golden();
    for len in 0..bytes.len() {
        assert!(acli::decode(&bytes[..len]).is_err(), "accepted {len} bytes");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        acli::decode(&trailing),
        Err(Error::TrailingBytes { extra: 1 })
    );
}

#[test]
fn validates_appearance_length_header_counts_and_reserved() {
    // Header + fixed base + one legacy texture override.
    const SIZE: usize = 16 + 60 + 6;
    let bytes = golden();
    assert_eq!(&bytes[SIZE..SIZE + 4], &36u32.to_le_bytes());
    for len in [1u32, 11, 12, 35, 37, u32::MAX] {
        let mut bad = bytes.clone();
        bad[SIZE..SIZE + 4].copy_from_slice(&len.to_le_bytes());
        assert!(
            acli::decode(&bad).is_err(),
            "accepted appearance size {len}"
        );
    }
    let mut bad = bytes.clone();
    bad[SIZE + 4 + 10] = 1;
    assert_eq!(
        acli::decode(&bad),
        Err(Error::InvalidAppearance {
            record: 0,
            error: AppearanceError::ReservedNotZero(1),
        })
    );
    for count_offset in [4, 6, 8] {
        let mut bad = bytes.clone();
        bad[SIZE + 4 + count_offset..SIZE + 6 + count_offset]
            .copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(matches!(
            acli::decode(&bad),
            Err(Error::InvalidAppearance { record: 0, .. })
        ));
    }
}

#[test]
fn feature_bit_requires_a_length_even_when_appearance_is_absent() {
    let value = acli::decode(&golden()).unwrap();
    let mut absent = value.clone();
    absent.instances.truncate(1);
    absent.instances[0].appearance = None;
    absent.instances[0].texture_overrides.clear();
    let mut bytes = acli::encode(&absent);
    bytes[6] = 1;
    assert_eq!(
        acli::decode(&bytes),
        Err(Error::TruncatedAppearanceLength {
            record: 0,
            available: 0
        })
    );
    bytes.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(acli::decode(&bytes).unwrap(), absent);
    // Encoders derive features from actual contents (same behavior as TS).
    assert_eq!(acli::encode(&absent)[6], 0);
}

#[test]
fn unknown_features_and_extreme_counts_fail_without_allocating_from_count() {
    for features in [4u16, 5, 0x8000, 0xffff] {
        let mut bad = golden();
        bad[6..8].copy_from_slice(&features.to_le_bytes());
        assert_eq!(acli::decode(&bad), Err(Error::UnknownFeatureBits(features)));
    }
    let mut bad = golden();
    bad[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(acli::decode(&bad).is_err());
}
