//! ACLI landblock-instances format (version 1).
//!
//! One file describes every server-side object the ACE world database places in
//! a single landblock (`landblock_instance`): the Meeting Hall mana pool, town
//! fountains, doors, portals, chests, lifestones, and similar static props that
//! are *not* in the client DATs. The generator pre-resolves each instance's
//! model, placement, and appearance offline; this module is the single Rust
//! implementation of the byte layout.
//!
//! The byte-level layout is specified in `docs/landblock-instances-format.md`.
//! This module is pure (no I/O) so the Worker and the CLI share it.

use crate::appearance::{self, Appearance, AppearanceError};
use std::error::Error;
use std::fmt;

/// File magic: ASCII `ACLI` as little-endian bytes.
pub const MAGIC: u32 = u32::from_le_bytes(*b"ACLI");
/// Format version written by this crate.
pub const VERSION: u8 = 1;
/// The one-byte kind field for landblock instances.
pub const KIND: u8 = 1;
/// Fixed header size in bytes.
pub const HEADER_LEN: usize = 16;
/// Fixed portion of one record, before texture overrides.
///
/// Holds a `u32 motionTableId` at offset `0x30` and a `u32 physicsScriptId` at
/// `0x34`, then `u32 guid` at `0x38`; texture overrides start at `0x3C`.
pub const RECORD_BASE: usize = 60;
/// Header feature mask value 1 (bit 0): every record has a sized appearance tail.
pub const ACLI_FEATURE_APPEARANCE: u16 = 0x0001;
/// Size of one texture override: `u16 partIndex` + `u32 newTextureId`.
pub const OVERRIDE_LEN: usize = 6;

/// `flags` bit 0: the instance is a `landblock_instance.is_Link_Child` row.
pub const LANDLOCK_INSTANCE_FLAG_LINK_CHILD: u8 = 0x01;

/// One texture-map override applied to a Setup part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureOverride {
    /// Setup part index the override applies to.
    pub part_index: u16,
    /// Replacement texture (0x06) id.
    pub texture_id: u32,
}

/// One placed object in a landblock.
#[derive(Clone, Debug, PartialEq)]
pub struct LandblockInstance {
    /// The `0x0200xxxx` model id.
    pub setup_id: u32,
    /// Landblock-local AC coordinates.
    pub origin: [f32; 3],
    /// Orientation quaternion `[W, X, Y, Z]`.
    pub orientation: [f32; 4],
    /// Weenie `DefaultScale`, `1.0` when absent.
    pub scale: f32,
    /// Sub-palette id, `0` = none.
    pub palette_id: u32,
    /// Palette offset, multiplied by 8 when applied.
    pub palette_offset: u16,
    /// Palette length, multiplied by 8 when applied.
    pub palette_length: u16,
    /// ACE WeenieType, for client filtering.
    pub weenie_type: u8,
    /// Bit 0 = `is_Link_Child`; the rest are reserved.
    pub flags: u8,
    /// Weenie `MotionTable` (0x09) DAT id, `0` = none.
    pub motion_table_id: u32,
    /// Weenie `PhysicsScript` (0x33) DAT id, `0` = none.
    pub physics_script_id: u32,
    /// Source instance GUID; zero when absent.
    pub guid: u32,
    /// Texture-map overrides, in source order.
    pub texture_overrides: Vec<TextureOverride>,
    /// Resolved ordered ObjDesc operations; independent of legacy overrides.
    pub appearance: Option<Appearance>,
}

/// A decoded or to-be-encoded landblock-instances file.
#[derive(Clone, Debug, PartialEq)]
pub struct LandblockInstances {
    /// 16-bit landblock id.
    pub landblock: u16,
    /// Instances in the order they were written (sorted by source `guid`).
    pub instances: Vec<LandblockInstance>,
}

impl LandblockInstances {
    /// Feature mask derived from the contents, matching the TypeScript encoder.
    pub fn feature_bits(&self) -> u16 {
        if self
            .instances
            .iter()
            .any(|instance| instance.appearance.is_some())
        {
            ACLI_FEATURE_APPEARANCE
        } else {
            0
        }
    }

    /// Total encoded size in bytes.
    pub fn encoded_len(&self) -> usize {
        let has_appearance = self.feature_bits() & ACLI_FEATURE_APPEARANCE != 0;
        HEADER_LEN
            + self
                .instances
                .iter()
                .map(|instance| {
                    RECORD_BASE
                        + instance.texture_overrides.len() * OVERRIDE_LEN
                        + if has_appearance {
                            4 + instance
                                .appearance
                                .as_ref()
                                .map_or(0, Appearance::encoded_len)
                        } else {
                            0
                        }
                })
                .sum::<usize>()
    }
}

/// Everything that can go wrong decoding a landblock-instances file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LandblockInstancesError {
    /// Fewer bytes than the fixed header requires.
    TooShort {
        needed: usize,
        actual: usize,
    },
    /// The first four bytes were not [`MAGIC`].
    BadMagic(u32),
    /// The version byte is not a version this crate understands.
    UnsupportedVersion(u8),
    /// The kind byte is not [`KIND`].
    UnknownKind(u8),
    /// Header features contain bits this codec does not recognize.
    UnknownFeatureBits(u16),
    TruncatedAppearanceLength {
        record: usize,
        available: usize,
    },
    TruncatedAppearance {
        record: usize,
        needed: usize,
        available: usize,
    },
    InvalidAppearance {
        record: usize,
        error: AppearanceError,
    },
    /// The landblock field did not fit in 16 bits.
    LandblockOutOfRange(u32),
    /// The fixed portion of a record runs past the end of the input.
    TruncatedRecord {
        index: usize,
        needed: usize,
        available: usize,
    },
    /// A texture override runs past the end of the input.
    TruncatedOverride {
        record: usize,
        index: usize,
        needed: usize,
        available: usize,
    },
    /// There were bytes left over after the last record.
    TrailingBytes {
        extra: usize,
    },
}

impl fmt::Display for LandblockInstancesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LandblockInstancesError::TooShort { needed, actual } => write!(
                f,
                "landblock instances file too short: need {needed} bytes, got {actual}"
            ),
            LandblockInstancesError::BadMagic(magic) => write!(
                f,
                "bad landblock instances magic \"{}\", expected \"ACLI\"",
                magic_tag(*magic)
            ),
            LandblockInstancesError::UnsupportedVersion(version) => {
                write!(f, "unsupported landblock instances version {version}")
            }
            LandblockInstancesError::UnknownKind(kind) => {
                write!(f, "unknown landblock instances kind {kind}")
            }
            LandblockInstancesError::UnknownFeatureBits(value) => {
                write!(f, "unknown ACLI feature bits: {value:#06x}")
            }
            LandblockInstancesError::TruncatedAppearanceLength { record, available } => {
                write!(f, "appearance size of record {record} needs 4 bytes, only {available} available")
            }
            LandblockInstancesError::TruncatedAppearance { record, needed, available } => {
                write!(f, "appearance of record {record} needs {needed} bytes, only {available} available")
            }
            LandblockInstancesError::InvalidAppearance { record, error } => {
                write!(f, "invalid appearance in record {record}: {error}")
            }
            LandblockInstancesError::LandblockOutOfRange(value) => {
                write!(f, "landblock {value} does not fit in 16 bits")
            }
            LandblockInstancesError::TruncatedRecord {
                index,
                needed,
                available,
            } => write!(
                f,
                "record {index} needs {needed} bytes, only {available} available"
            ),
            LandblockInstancesError::TruncatedOverride {
                record,
                index,
                needed,
                available,
            } => write!(
                f,
                "texture override {index} of record {record} needs {needed} bytes, only {available} available"
            ),
            LandblockInstancesError::TrailingBytes { extra } => {
                write!(f, "{extra} trailing bytes after the last record")
            }
        }
    }
}

impl Error for LandblockInstancesError {}

/// Encode a landblock-instances file in the current 60-byte layout.
///
/// Panics if an instance/operation count cannot fit its wire field.
pub fn encode(instances: &LandblockInstances) -> Vec<u8> {
    let count = u32::try_from(instances.instances.len()).expect("too many ACLI instances");
    let features = instances.feature_bits();
    let mut out = Vec::with_capacity(instances.encoded_len());

    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(VERSION);
    out.push(KIND);
    out.extend_from_slice(&features.to_le_bytes());
    out.extend_from_slice(&(instances.landblock as u32).to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());

    for instance in &instances.instances {
        out.extend_from_slice(&instance.setup_id.to_le_bytes());
        for component in instance.origin {
            out.extend_from_slice(&component.to_le_bytes());
        }
        for component in instance.orientation {
            out.extend_from_slice(&component.to_le_bytes());
        }
        out.extend_from_slice(&instance.scale.to_le_bytes());
        out.extend_from_slice(&instance.palette_id.to_le_bytes());
        out.extend_from_slice(&instance.palette_offset.to_le_bytes());
        out.extend_from_slice(&instance.palette_length.to_le_bytes());
        out.push(instance.weenie_type);
        out.push(instance.flags);
        let texture_count = u16::try_from(instance.texture_overrides.len())
            .expect("too many ACLI texture overrides");
        out.extend_from_slice(&texture_count.to_le_bytes());
        out.extend_from_slice(&instance.motion_table_id.to_le_bytes());
        out.extend_from_slice(&instance.physics_script_id.to_le_bytes());
        out.extend_from_slice(&instance.guid.to_le_bytes());
        for texture_override in &instance.texture_overrides {
            out.extend_from_slice(&texture_override.part_index.to_le_bytes());
            out.extend_from_slice(&texture_override.texture_id.to_le_bytes());
        }
        if features & ACLI_FEATURE_APPEARANCE != 0 {
            let bytes = instance
                .appearance
                .as_ref()
                .map(appearance::encode)
                .transpose()
                .expect("appearance operation count exceeds u16")
                .unwrap_or_default();
            let len = u32::try_from(bytes.len()).expect("appearance payload exceeds u32");
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&bytes);
        }
    }

    out
}

/// Decode a landblock-instances file, validating every field and bound.
pub fn decode(bytes: &[u8]) -> Result<LandblockInstances, LandblockInstancesError> {
    if bytes.len() < HEADER_LEN {
        return Err(LandblockInstancesError::TooShort {
            needed: HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let magic = read_u32(bytes, 0);
    if magic != MAGIC {
        return Err(LandblockInstancesError::BadMagic(magic));
    }

    if bytes[4] != VERSION {
        return Err(LandblockInstancesError::UnsupportedVersion(bytes[4]));
    }

    let kind = bytes[5];
    if kind != KIND {
        return Err(LandblockInstancesError::UnknownKind(kind));
    }

    let features = read_u16(bytes, 6);
    if features & !ACLI_FEATURE_APPEARANCE != 0 {
        return Err(LandblockInstancesError::UnknownFeatureBits(features));
    }

    let raw_landblock = read_u32(bytes, 8);
    let landblock = u16::try_from(raw_landblock)
        .map_err(|_| LandblockInstancesError::LandblockOutOfRange(raw_landblock))?;

    let count = read_u32(bytes, 12) as usize;

    let mut cursor = HEADER_LEN;
    // Do not allocate from an untrusted u32 count before checking record bounds.
    let mut instances = Vec::new();
    for index in 0..count {
        let available = bytes.len().saturating_sub(cursor);
        if available < RECORD_BASE {
            return Err(LandblockInstancesError::TruncatedRecord {
                index,
                needed: RECORD_BASE,
                available,
            });
        }

        let setup_id = read_u32(bytes, cursor);
        let origin = read_f32x3(bytes, cursor + 4);
        let orientation = read_f32x4(bytes, cursor + 16);
        let scale = read_f32(bytes, cursor + 32);
        let palette_id = read_u32(bytes, cursor + 36);
        let palette_offset = read_u16(bytes, cursor + 40);
        let palette_length = read_u16(bytes, cursor + 42);
        let weenie_type = bytes[cursor + 44];
        let flags = bytes[cursor + 45];
        let texture_count = read_u16(bytes, cursor + 46) as usize;
        let motion_table_id = read_u32(bytes, cursor + 48);
        let physics_script_id = read_u32(bytes, cursor + 52);
        let guid = read_u32(bytes, cursor + 56);
        cursor += RECORD_BASE;

        let mut texture_overrides = Vec::with_capacity(texture_count);
        for override_index in 0..texture_count {
            let available = bytes.len().saturating_sub(cursor);
            if available < OVERRIDE_LEN {
                return Err(LandblockInstancesError::TruncatedOverride {
                    record: index,
                    index: override_index,
                    needed: OVERRIDE_LEN,
                    available,
                });
            }
            texture_overrides.push(TextureOverride {
                part_index: read_u16(bytes, cursor),
                texture_id: read_u32(bytes, cursor + 2),
            });
            cursor += OVERRIDE_LEN;
        }

        let appearance = if features & ACLI_FEATURE_APPEARANCE != 0 {
            let available = bytes.len().saturating_sub(cursor);
            if available < 4 {
                return Err(LandblockInstancesError::TruncatedAppearanceLength {
                    record: index,
                    available,
                });
            }
            let size = read_u32(bytes, cursor) as usize;
            cursor += 4;
            let available = bytes.len().saturating_sub(cursor);
            if size > available {
                return Err(LandblockInstancesError::TruncatedAppearance {
                    record: index,
                    needed: size,
                    available,
                });
            }
            let value = if size == 0 {
                None
            } else {
                Some(
                    appearance::decode(&bytes[cursor..cursor + size]).map_err(|error| {
                        LandblockInstancesError::InvalidAppearance {
                            record: index,
                            error,
                        }
                    })?,
                )
            };
            cursor += size;
            value
        } else {
            None
        };

        instances.push(LandblockInstance {
            setup_id,
            origin,
            orientation,
            scale,
            palette_id,
            palette_offset,
            palette_length,
            weenie_type,
            flags,
            motion_table_id,
            physics_script_id,
            guid,
            texture_overrides,
            appearance,
        });
    }

    if cursor < bytes.len() {
        return Err(LandblockInstancesError::TrailingBytes {
            extra: bytes.len() - cursor,
        });
    }

    Ok(LandblockInstances {
        landblock,
        instances,
    })
}

/// Render a magic value as its four little-endian ASCII bytes, replacing
/// non-printable bytes with dots.
fn magic_tag(magic: u32) -> String {
    magic
        .to_le_bytes()
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() {
                byte as char
            } else {
                '.'
            }
        })
        .collect()
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_f32(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_f32x3(bytes: &[u8], at: usize) -> [f32; 3] {
    [
        read_f32(bytes, at),
        read_f32(bytes, at + 4),
        read_f32(bytes, at + 8),
    ]
}

fn read_f32x4(bytes: &[u8], at: usize) -> [f32; 4] {
    [
        read_f32(bytes, at),
        read_f32(bytes, at + 4),
        read_f32(bytes, at + 8),
        read_f32(bytes, at + 12),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(setup_id: u32, overrides: Vec<TextureOverride>) -> LandblockInstance {
        LandblockInstance {
            setup_id,
            origin: [1.0, 2.0, 3.0],
            orientation: [0.5, 0.25, 0.125, 0.0625],
            scale: 1.5,
            palette_id: 0x0400_1234,
            palette_offset: 7,
            palette_length: 9,
            weenie_type: 42,
            flags: LANDLOCK_INSTANCE_FLAG_LINK_CHILD,
            motion_table_id: 0,
            physics_script_id: 0,
            guid: 0,
            texture_overrides: overrides,
            appearance: None,
        }
    }

    fn plain_instances() -> LandblockInstances {
        LandblockInstances {
            landblock: 0xB34A,
            instances: vec![
                instance(0x0200_0001, Vec::new()),
                instance(0x0200_0002, Vec::new()),
            ],
        }
    }

    fn textured_instances() -> LandblockInstances {
        let mut inst = instance(
            0x0200_0003,
            vec![
                TextureOverride {
                    part_index: 3,
                    texture_id: 0x0600_ABCD,
                },
                TextureOverride {
                    part_index: 0,
                    texture_id: 0x0600_0001,
                },
            ],
        );
        inst.motion_table_id = 0x0900_0007;
        inst.physics_script_id = 0x3300_0005;
        LandblockInstances {
            landblock: 0x00AE,
            instances: vec![inst],
        }
    }

    #[test]
    fn roundtrips_instances_without_overrides() {
        let instances = plain_instances();
        let decoded = decode(&encode(&instances)).unwrap();
        assert_eq!(decoded, instances);
    }

    #[test]
    fn roundtrips_instances_with_overrides() {
        let instances = textured_instances();
        let decoded = decode(&encode(&instances)).unwrap();
        assert_eq!(decoded, instances);
    }

    #[test]
    fn roundtrips_empty_instances() {
        let instances = LandblockInstances {
            landblock: 0x0000,
            instances: Vec::new(),
        };
        let encoded = encode(&instances);
        assert_eq!(encoded.len(), HEADER_LEN);
        assert_eq!(decode(&encoded).unwrap(), instances);
    }

    #[test]
    fn header_layout_is_little_endian() {
        let encoded = encode(&LandblockInstances {
            landblock: 0x0125,
            instances: Vec::new(),
        });

        #[rustfmt::skip]
        let expected: [u8; HEADER_LEN] = [
            0x41, 0x43, 0x4C, 0x49, // magic "ACLI"
            0x01,                   // version
            0x01,                   // kind = landblock instances
            0x00, 0x00,             // reserved
            0x25, 0x01, 0x00, 0x00, // landblock = 0x0125
            0x00, 0x00, 0x00, 0x00, // instanceCount = 0
        ];

        assert_eq!(encoded, expected);
        assert_eq!(&encoded[0..4], b"ACLI");
        assert_eq!(encoded[4], VERSION);
        assert_eq!(encoded[5], KIND);
        assert_eq!(&encoded[6..8], &[0, 0]);
    }

    #[test]
    fn record_layout_is_little_endian() {
        let instances = textured_instances();
        let encoded = encode(&instances);
        assert_eq!(encoded.len(), HEADER_LEN + RECORD_BASE + 2 * OVERRIDE_LEN);

        let record = HEADER_LEN;
        assert_eq!(&encoded[record..record + 4], &0x0200_0003u32.to_le_bytes());
        assert_eq!(&encoded[record + 4..record + 8], &1.0f32.to_le_bytes());
        assert_eq!(&encoded[record + 8..record + 12], &2.0f32.to_le_bytes());
        assert_eq!(&encoded[record + 12..record + 16], &3.0f32.to_le_bytes());
        assert_eq!(&encoded[record + 16..record + 20], &0.5f32.to_le_bytes());
        assert_eq!(&encoded[record + 20..record + 24], &0.25f32.to_le_bytes());
        assert_eq!(&encoded[record + 24..record + 28], &0.125f32.to_le_bytes());
        assert_eq!(&encoded[record + 28..record + 32], &0.0625f32.to_le_bytes());
        assert_eq!(&encoded[record + 32..record + 36], &1.5f32.to_le_bytes());
        assert_eq!(
            &encoded[record + 36..record + 40],
            &0x0400_1234u32.to_le_bytes()
        );
        assert_eq!(&encoded[record + 40..record + 42], &7u16.to_le_bytes());
        assert_eq!(&encoded[record + 42..record + 44], &9u16.to_le_bytes());
        assert_eq!(encoded[record + 44], 42);
        assert_eq!(encoded[record + 45], LANDLOCK_INSTANCE_FLAG_LINK_CHILD);
        assert_eq!(&encoded[record + 46..record + 48], &2u16.to_le_bytes());
        // motionTableId sits at record offset 0x30.
        assert_eq!(
            &encoded[record + 0x30..record + 0x34],
            &0x0900_0007u32.to_le_bytes()
        );
        // physicsScriptId sits at record offset 0x34.
        assert_eq!(
            &encoded[record + 0x34..record + 0x38],
            &0x3300_0005u32.to_le_bytes()
        );

        // guid occupies 0x38, so texture overrides start at 0x3C.
        assert_eq!(&encoded[record + 0x38..record + 0x3C], &0u32.to_le_bytes());
        let overrides = record + RECORD_BASE;
        assert_eq!(overrides, record + 0x3C);
        assert_eq!(&encoded[overrides..overrides + 2], &3u16.to_le_bytes());
        assert_eq!(
            &encoded[overrides + 2..overrides + 6],
            &0x0600_ABCDu32.to_le_bytes()
        );
        assert_eq!(&encoded[overrides + 6..overrides + 8], &0u16.to_le_bytes());
        assert_eq!(
            &encoded[overrides + 8..overrides + 12],
            &0x0600_0001u32.to_le_bytes()
        );
    }

    #[test]
    fn roundtrips_nonzero_motion_table_id() {
        let instances = LandblockInstances {
            landblock: 0x00AE,
            instances: vec![LandblockInstance {
                motion_table_id: 0x0900_0664,
                ..instance(0x0200_0042, Vec::new())
            }],
        };
        let encoded = encode(&instances);
        assert_eq!(
            &encoded[HEADER_LEN + 0x30..HEADER_LEN + 0x34],
            &0x0900_0664u32.to_le_bytes()
        );
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, instances);
        assert_eq!(decoded.instances[0].motion_table_id, 0x0900_0664);
    }

    #[test]
    fn roundtrips_nonzero_physics_script_id() {
        let instances = LandblockInstances {
            landblock: 0x00AE,
            instances: vec![LandblockInstance {
                physics_script_id: 0x3300_1234,
                ..instance(0x0200_0043, Vec::new())
            }],
        };
        let encoded = encode(&instances);
        assert_eq!(
            &encoded[HEADER_LEN + 0x34..HEADER_LEN + 0x38],
            &0x3300_1234u32.to_le_bytes()
        );
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, instances);
        assert_eq!(decoded.instances[0].physics_script_id, 0x3300_1234);
    }

    #[test]
    fn worked_example_matches_docs() {
        // Block 0x0125, 2 records, no overrides: header + 2 * 60 bytes.
        let instances = LandblockInstances {
            landblock: 0x0125,
            instances: vec![
                LandblockInstance {
                    setup_id: 0,
                    origin: [0.0; 3],
                    orientation: [0.0; 4],
                    scale: 1.0,
                    palette_id: 0,
                    palette_offset: 0,
                    palette_length: 0,
                    weenie_type: 0,
                    flags: 0,
                    motion_table_id: 0,
                    physics_script_id: 0,
                    guid: 0,
                    texture_overrides: Vec::new(),
                    appearance: None,
                },
                LandblockInstance {
                    setup_id: 0,
                    origin: [0.0; 3],
                    orientation: [0.0; 4],
                    scale: 1.0,
                    palette_id: 0,
                    palette_offset: 0,
                    palette_length: 0,
                    weenie_type: 0,
                    flags: 0,
                    motion_table_id: 0,
                    physics_script_id: 0,
                    guid: 0,
                    texture_overrides: Vec::new(),
                    appearance: None,
                },
            ],
        };

        let encoded = encode(&instances);
        assert_eq!(encoded.len(), HEADER_LEN + 2 * RECORD_BASE);

        #[rustfmt::skip]
        let expected_header: [u8; HEADER_LEN] = [
            0x41, 0x43, 0x4C, 0x49, // magic "ACLI"
            0x01,                   // version
            0x01,                   // kind = landblock instances
            0x00, 0x00,             // reserved
            0x25, 0x01, 0x00, 0x00, // landblock = 0x0125
            0x02, 0x00, 0x00, 0x00, // instanceCount = 2
        ];
        assert_eq!(&encoded[..HEADER_LEN], &expected_header);
        assert_eq!(decode(&encoded).unwrap(), instances);
    }

    #[test]
    fn encoded_len_matches_encode() {
        for instances in [plain_instances(), textured_instances()] {
            assert_eq!(instances.encoded_len(), encode(&instances).len());
        }
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            decode(&[0u8; 8]),
            Err(LandblockInstancesError::TooShort {
                needed: HEADER_LEN,
                actual: 8
            })
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut encoded = encode(&plain_instances());
        encoded[0] = b'X';
        assert!(matches!(
            decode(&encoded),
            Err(LandblockInstancesError::BadMagic(_))
        ));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut encoded = encode(&plain_instances());
        encoded[4] = 0xFF;
        assert_eq!(
            decode(&encoded),
            Err(LandblockInstancesError::UnsupportedVersion(0xFF))
        );
    }

    #[test]
    fn rejects_unknown_kind() {
        let mut encoded = encode(&plain_instances());
        encoded[5] = 2;
        assert_eq!(
            decode(&encoded),
            Err(LandblockInstancesError::UnknownKind(2))
        );
    }

    #[test]
    fn rejects_unknown_features() {
        let mut encoded = encode(&plain_instances());
        encoded[6..8].copy_from_slice(&0x8002u16.to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(LandblockInstancesError::UnknownFeatureBits(0x8002))
        );
    }

    #[test]
    fn rejects_landblock_that_does_not_fit_u16() {
        let mut encoded = encode(&plain_instances());
        encoded[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(LandblockInstancesError::LandblockOutOfRange(0x0001_0000))
        );
    }

    #[test]
    fn rejects_truncated_record() {
        let encoded = encode(&plain_instances());
        // Keep the header (which promises 2 records) but drop the records.
        assert_eq!(
            decode(&encoded[..HEADER_LEN]),
            Err(LandblockInstancesError::TruncatedRecord {
                index: 0,
                needed: RECORD_BASE,
                available: 0
            })
        );
    }

    #[test]
    fn rejects_truncated_second_record() {
        let encoded = encode(&plain_instances());
        let truncated = &encoded[..HEADER_LEN + RECORD_BASE + RECORD_BASE - 1];
        assert_eq!(
            decode(truncated),
            Err(LandblockInstancesError::TruncatedRecord {
                index: 1,
                needed: RECORD_BASE,
                available: RECORD_BASE - 1
            })
        );
    }

    #[test]
    fn rejects_truncated_override() {
        let encoded = encode(&textured_instances());
        let truncated = &encoded[..encoded.len() - 1];
        assert_eq!(
            decode(truncated),
            Err(LandblockInstancesError::TruncatedOverride {
                record: 0,
                index: 1,
                needed: OVERRIDE_LEN,
                available: OVERRIDE_LEN - 1
            })
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut encoded = encode(&textured_instances());
        encoded.push(0);
        assert_eq!(
            decode(&encoded),
            Err(LandblockInstancesError::TrailingBytes { extra: 1 })
        );
    }

    #[test]
    fn error_messages_are_descriptive() {
        assert!(LandblockInstancesError::BadMagic(0)
            .to_string()
            .contains("magic"));
        assert_eq!(magic_tag(MAGIC), "ACLI");
        assert!(LandblockInstancesError::UnknownKind(9)
            .to_string()
            .contains("kind"));
        assert!(LandblockInstancesError::TruncatedRecord {
            index: 0,
            needed: RECORD_BASE,
            available: 0
        }
        .to_string()
        .contains("record 0"));
        assert!(LandblockInstancesError::TrailingBytes { extra: 3 }
            .to_string()
            .contains('3'));
    }
}
