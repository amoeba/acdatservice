use acdatservice::{
    appearance::{self, Appearance, AppearanceError, HeldAttachment},
    landblock_instances::{self as acli, LandblockInstances, LandblockInstancesError},
};

fn unhex(hex: &str) -> Vec<u8> {
    let hex = hex.trim();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

fn child(appearance: Option<Appearance>) -> HeldAttachment {
    HeldAttachment {
        setup_id: 0x0200_0002,
        parent_location: 1,
        placement_id: 1,
        scale: 0.75,
        appearance,
    }
}

fn chain(edges: usize) -> Appearance {
    if edges == 0 {
        return Appearance::default();
    }
    Appearance {
        attachments: vec![child(Some(chain(edges - 1)))],
        ..Appearance::default()
    }
}

fn dataset(appearance: Appearance) -> LandblockInstances {
    let mut value = acli::decode(&unhex(include_str!("fixtures/acli-appearance.hex"))).unwrap();
    value.instances[0].appearance = Some(appearance);
    value
}

#[test]
fn typescript_attachment_golden_matches_rust_bytes() {
    // Independently produced by ac-noclip encodeAcli, with input JSON alongside.
    let bytes = unhex(include_str!("fixtures/acli-attachments.hex"));
    let value = acli::decode(&bytes).unwrap();
    assert_eq!(value.landblock, 0x1234);
    assert_eq!(value.feature_bits(), 3);
    assert_eq!(value.instances.len(), 3);
    assert_eq!(value.instances[0].appearance, None);
    assert_eq!(value.instances[1].appearance, Some(Appearance::default()));
    assert_eq!(
        value.instances[2].appearance,
        Some(Appearance {
            attachments: vec![HeldAttachment {
                setup_id: 0x0200_0123,
                parent_location: 3,
                placement_id: 6,
                scale: 1.25,
                appearance: Some(Appearance {
                    palette_base_id: 0x0400_007e,
                    attachments: vec![HeldAttachment {
                        setup_id: 0x0200_0456,
                        parent_location: 1,
                        placement_id: 1,
                        scale: 0.5,
                        appearance: None,
                    }],
                    ..Appearance::default()
                }),
            }],
            ..Appearance::default()
        })
    );
    assert_eq!(value.encoded_len(), bytes.len());
    assert_eq!(acli::encode(&value), bytes);
    let json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/acli-attachments.json")).unwrap();
    assert_eq!(json["landblock"], value.landblock);
    for (i, instance) in value.instances.iter().enumerate() {
        assert_eq!(instance.setup_id, 0x0200_0001);
        assert_eq!(instance.origin, [1.0, 2.0, 3.0]);
        assert_eq!(instance.orientation, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(instance.scale, 1.0);
        assert_eq!(instance.flags, 2);
        assert_eq!(instance.guid, (i + 1) as u32);
    }
}

#[test]
fn mask3_keeps_all_base_placements_and_feature1_readable() {
    let mut value = dataset(chain(2));
    let bytes = acli::encode(&value);
    assert_eq!(value.feature_bits(), 3);
    assert_eq!(value.encoded_len(), bytes.len());
    assert_eq!(acli::decode(&bytes).unwrap(), value);
    let legacy = unhex(include_str!("fixtures/acli-appearance.hex"));
    let previous = acli::decode(&legacy).unwrap();
    for (got, old) in value.instances.iter().zip(previous.instances.iter()) {
        let mut got = got.clone();
        got.appearance = old.appearance.clone();
        assert_eq!(&got, old); // every existing placement field is unchanged
    }
    value.instances[0]
        .appearance
        .as_mut()
        .unwrap()
        .attachments
        .clear();
    assert_eq!(acli::encode(&value)[6], 1);
    assert_eq!(acli::encode(&previous), legacy);
}

#[test]
fn attachments_need_feature1_and_legacy_reserved_word_stays_strict() {
    let value = chain(1);
    assert_eq!(
        appearance::encode_with_attachments(&value, false),
        Err(AppearanceError::AttachmentsDisabled)
    );
    let bytes = appearance::encode_with_attachments(&value, true).unwrap();
    assert_eq!(
        appearance::decode_with_attachments(&bytes, false),
        Err(AppearanceError::ReservedNotZero(1))
    );
    assert_eq!(
        appearance::decode(&appearance::encode(&value).unwrap()).unwrap(),
        value
    );
    let mut bytes = acli::encode(&dataset(value));
    bytes[6] = 2;
    assert_eq!(
        acli::decode(&bytes),
        Err(LandblockInstancesError::AttachmentsRequireAppearance)
    );
    bytes[6] = 1;
    assert!(matches!(
        acli::decode(&bytes),
        Err(LandblockInstancesError::InvalidAppearance { .. })
    ));
}

#[test]
fn every_attachment_truncation_and_trailing_byte_fails() {
    let value = chain(2);
    let bytes = appearance::encode_with_attachments(&value, true).unwrap();
    for end in 0..bytes.len() {
        assert!(
            appearance::decode_with_attachments(&bytes[..end], true).is_err(),
            "accepted {end}"
        );
    }
    let bytes = acli::encode(&dataset(value));
    for end in 0..bytes.len() {
        assert!(acli::decode(&bytes[..end]).is_err(), "accepted ACLI {end}");
    }
    let mut bytes = appearance::encode_with_attachments(&chain(1), true).unwrap();
    bytes.push(0);
    assert!(appearance::decode_with_attachments(&bytes, true).is_err());
    // Child length must not consume neighboring records or silently ignore bytes.
    for size in [1u32, 11, 13, u32::MAX] {
        let mut bytes = appearance::encode_with_attachments(&chain(1), true).unwrap();
        bytes[24..28].copy_from_slice(&size.to_le_bytes());
        assert!(appearance::decode_with_attachments(&bytes, true).is_err());
    }
}

#[test]
fn validates_attachment_identifiers_and_f32_scales_on_both_paths() {
    let base = child(None);
    let mut invalid = Vec::new();
    for setup_id in [0, 0x0100_0002, u32::MAX] {
        invalid.push(HeldAttachment {
            setup_id,
            ..base.clone()
        });
    }
    for parent_location in [0, 4, 7, 9, u16::MAX] {
        invalid.push(HeldAttachment {
            parent_location,
            ..base.clone()
        });
    }
    for placement_id in [0, 4, 5, 7, u16::MAX] {
        invalid.push(HeldAttachment {
            placement_id,
            ..base.clone()
        });
    }
    for scale in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        invalid.push(HeldAttachment {
            scale,
            ..base.clone()
        });
    }
    for attachment in invalid {
        let value = Appearance {
            attachments: vec![attachment.clone()],
            ..Appearance::default()
        };
        assert!(appearance::encode_with_attachments(&value, true).is_err());
        let mut bytes = appearance::encode_with_attachments(
            &Appearance {
                attachments: vec![base.clone()],
                ..Appearance::default()
            },
            true,
        )
        .unwrap();
        bytes[12..16].copy_from_slice(&attachment.setup_id.to_le_bytes());
        bytes[16..18].copy_from_slice(&attachment.parent_location.to_le_bytes());
        bytes[18..20].copy_from_slice(&attachment.placement_id.to_le_bytes());
        bytes[20..24].copy_from_slice(&attachment.scale.to_le_bytes());
        assert!(appearance::decode_with_attachments(&bytes, true).is_err());
    }
    for parent_location in [1, 2, 3, 8] {
        for placement_id in [1, 2, 3, 6] {
            let value = Appearance {
                attachments: vec![HeldAttachment {
                    parent_location,
                    placement_id,
                    scale: f32::MIN_POSITIVE,
                    ..base.clone()
                }],
                ..Appearance::default()
            };
            let bytes = appearance::encode_with_attachments(&value, true).unwrap();
            assert_eq!(
                appearance::decode_with_attachments(&bytes, true).unwrap(),
                value
            );
        }
    }
}

// Construct wire bytes independently so decoder rejection isn't just encoder validation.
fn unchecked_wire(value: &Appearance) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    bytes[10..12].copy_from_slice(&(value.attachments.len() as u16).to_le_bytes());
    for child in &value.attachments {
        let nested = child
            .appearance
            .as_ref()
            .map(unchecked_wire)
            .unwrap_or_default();
        bytes.extend_from_slice(&child.setup_id.to_le_bytes());
        bytes.extend_from_slice(&child.parent_location.to_le_bytes());
        bytes.extend_from_slice(&child.placement_id.to_le_bytes());
        bytes.extend_from_slice(&child.scale.to_le_bytes());
        bytes.extend_from_slice(&(nested.len() as u32).to_le_bytes());
        bytes.extend(nested);
    }
    bytes
}

#[test]
fn depth_four_and_total_32_are_inclusive_and_budgets_are_per_root() {
    for value in [
        chain(4),
        Appearance {
            attachments: vec![child(None); 32],
            ..Appearance::default()
        },
    ] {
        let bytes = appearance::encode_with_attachments(&value, true).unwrap();
        assert_eq!(
            appearance::decode_with_attachments(&bytes, true).unwrap(),
            value
        );
    }
    let nested_siblings = Appearance {
        attachments: vec![
            child(Some(Appearance {
                attachments: vec![child(None); 16],
                ..Appearance::default()
            }));
            2
        ],
        ..Appearance::default()
    }; // 34 nodes, not separate 17-node budgets
    for value in [
        chain(5),
        Appearance {
            attachments: vec![child(None); 33],
            ..Appearance::default()
        },
        nested_siblings,
    ] {
        assert!(appearance::encode_with_attachments(&value, true).is_err());
        assert!(appearance::decode_with_attachments(&unchecked_wire(&value), true).is_err());
    }
    // A leaf without child appearance still counts as an edge.
    let mut value = chain(4);
    let mut leaf = &mut value;
    for _ in 0..4 {
        leaf = leaf.attachments[0].appearance.as_mut().unwrap();
    }
    leaf.attachments.push(child(None));
    assert!(appearance::decode_with_attachments(&unchecked_wire(&value), true).is_err());
    let mut value = dataset(Appearance {
        attachments: vec![child(None); 32],
        ..Appearance::default()
    });
    value.instances[2].appearance = value.instances[0].appearance.clone();
    assert_eq!(acli::decode(&acli::encode(&value)).unwrap(), value);
}

#[test]
fn preserves_absent_empty_duplicate_setups_and_order() {
    let value = Appearance {
        attachments: vec![
            child(None),
            child(Some(Appearance::default())),
            child(Some(chain(1))),
        ],
        ..Appearance::default()
    };
    let bytes = appearance::encode_with_attachments(&value, true).unwrap();
    assert_eq!(
        appearance::decode_with_attachments(&bytes, true).unwrap(),
        value
    );
    let mut count_bomb = vec![0; 12];
    count_bomb[10..12].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(appearance::decode_with_attachments(&count_bomb, true).is_err());
}
