//! Resolved ACE ObjDesc operations carried by the ACLI appearance feature.
//!
//! Operations retain authored/layer order (later changes win). Palette ranges
//! are in blocks of eight colors. No renderer or database resolution happens here.

use std::{error::Error, fmt};

pub const HEADER_LEN: usize = 12;
pub const PART_LEN: usize = 6;
pub const TEXTURE_LEN: usize = 10;
pub const PALETTE_LEN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartChange {
    pub part_index: u16,
    /// GfxObj (0x01) DAT id.
    pub gfx_obj_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureChange {
    pub part_index: u16,
    /// SurfaceTexture (0x05) DAT ids; replacement is conditional on the old id.
    pub old_texture_id: u32,
    pub new_texture_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteChange {
    pub palette_id: u32,
    /// Range start in blocks of eight colors (not bytes or individual colors).
    pub offset: u16,
    /// Range length in blocks of eight colors, retained verbatim.
    pub length: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    /// Base palette (0x04) DAT id; zero means absent.
    pub palette_base_id: u32,
    pub parts: Vec<PartChange>,
    pub textures: Vec<TextureChange>,
    pub palettes: Vec<PaletteChange>,
}

impl Appearance {
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + self.parts.len() * PART_LEN
            + self.textures.len() * TEXTURE_LEN
            + self.palettes.len() * PALETTE_LEN
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppearanceError {
    TooShort { actual: usize },
    ReservedNotZero(u16),
    InvalidLength { expected: usize, actual: usize },
    TooManyOperations(&'static str),
}

impl fmt::Display for AppearanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { actual } => write!(
                f,
                "appearance header needs {HEADER_LEN} bytes, got {actual}"
            ),
            Self::ReservedNotZero(value) => {
                write!(f, "appearance reserved field must be zero, got {value}")
            }
            Self::InvalidLength { expected, actual } => write!(
                f,
                "appearance operations need exactly {expected} bytes, got {actual}"
            ),
            Self::TooManyOperations(kind) => write!(f, "appearance {kind} count exceeds u16"),
        }
    }
}

impl Error for AppearanceError {}

pub fn encode(value: &Appearance) -> Result<Vec<u8>, AppearanceError> {
    let parts =
        u16::try_from(value.parts.len()).map_err(|_| AppearanceError::TooManyOperations("part"))?;
    let textures = u16::try_from(value.textures.len())
        .map_err(|_| AppearanceError::TooManyOperations("texture"))?;
    let palettes = u16::try_from(value.palettes.len())
        .map_err(|_| AppearanceError::TooManyOperations("palette"))?;
    let mut out = Vec::with_capacity(value.encoded_len());
    out.extend_from_slice(&value.palette_base_id.to_le_bytes());
    out.extend_from_slice(&parts.to_le_bytes());
    out.extend_from_slice(&textures.to_le_bytes());
    out.extend_from_slice(&palettes.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    for part in &value.parts {
        out.extend_from_slice(&part.part_index.to_le_bytes());
        out.extend_from_slice(&part.gfx_obj_id.to_le_bytes());
    }
    for texture in &value.textures {
        out.extend_from_slice(&texture.part_index.to_le_bytes());
        out.extend_from_slice(&texture.old_texture_id.to_le_bytes());
        out.extend_from_slice(&texture.new_texture_id.to_le_bytes());
    }
    for palette in &value.palettes {
        out.extend_from_slice(&palette.palette_id.to_le_bytes());
        out.extend_from_slice(&palette.offset.to_le_bytes());
        out.extend_from_slice(&palette.length.to_le_bytes());
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Appearance, AppearanceError> {
    if bytes.len() < HEADER_LEN {
        return Err(AppearanceError::TooShort {
            actual: bytes.len(),
        });
    }
    let parts = read_u16(bytes, 4) as usize;
    let textures = read_u16(bytes, 6) as usize;
    let palettes = read_u16(bytes, 8) as usize;
    let reserved = read_u16(bytes, 10);
    if reserved != 0 {
        return Err(AppearanceError::ReservedNotZero(reserved));
    }
    let expected = HEADER_LEN + parts * PART_LEN + textures * TEXTURE_LEN + palettes * PALETTE_LEN;
    if expected != bytes.len() {
        return Err(AppearanceError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    // Validate the complete bounded payload before allocating from wire counts.
    let mut value = Appearance {
        palette_base_id: read_u32(bytes, 0),
        ..Appearance::default()
    };
    let mut at = HEADER_LEN;
    for _ in 0..parts {
        value.parts.push(PartChange {
            part_index: read_u16(bytes, at),
            gfx_obj_id: read_u32(bytes, at + 2),
        });
        at += PART_LEN;
    }
    for _ in 0..textures {
        value.textures.push(TextureChange {
            part_index: read_u16(bytes, at),
            old_texture_id: read_u32(bytes, at + 2),
            new_texture_id: read_u32(bytes, at + 6),
        });
        at += TEXTURE_LEN;
    }
    for _ in 0..palettes {
        value.palettes.push(PaletteChange {
            palette_id: read_u32(bytes, at),
            offset: read_u16(bytes, at + 4),
            length: read_u16(bytes, at + 6),
        });
        at += PALETTE_LEN;
    }
    Ok(value)
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Appearance {
        Appearance {
            palette_base_id: 0x0400_0001,
            parts: vec![PartChange {
                part_index: 3,
                gfx_obj_id: 0x0100_1234,
            }],
            textures: vec![TextureChange {
                part_index: 3,
                old_texture_id: 0x0500_0001,
                new_texture_id: 0x0500_0002,
            }],
            palettes: vec![PaletteChange {
                palette_id: 0x0400_0002,
                offset: 2,
                length: 4,
            }],
        }
    }

    #[test]
    fn little_endian_golden_matches_typescript_appearance() {
        let value = sample();
        #[rustfmt::skip]
        let expected = [
            1, 0, 0, 4, 1, 0, 1, 0, 1, 0, 0, 0,
            3, 0, 0x34, 0x12, 0, 1,
            3, 0, 1, 0, 0, 5, 2, 0, 0, 5,
            2, 0, 0, 4, 2, 0, 4, 0,
        ];
        assert_eq!(encode(&value).unwrap(), expected);
        assert_eq!(decode(&expected).unwrap(), value);
        assert_eq!(value.encoded_len(), expected.len());
    }

    #[test]
    fn empty_payload_is_distinct_from_absent_tail() {
        assert_eq!(encode(&Appearance::default()).unwrap(), [0; HEADER_LEN]);
        assert_eq!(decode(&[0; HEADER_LEN]).unwrap(), Appearance::default());
        assert!(decode(&[]).is_err());
    }

    #[test]
    fn preserves_duplicate_operations_order_and_block_ranges() {
        let mut value = sample();
        value.parts.push(PartChange {
            part_index: 3,
            gfx_obj_id: 0x0100_5678,
        });
        value.textures.push(value.textures[0]);
        value.palettes.push(PaletteChange {
            palette_id: u32::MAX,
            offset: u16::MAX,
            length: 0,
        });
        assert_eq!(decode(&encode(&value).unwrap()).unwrap(), value);
    }

    #[test]
    fn rejects_every_truncation_trailing_bytes_counts_and_reserved() {
        let bytes = encode(&sample()).unwrap();
        for len in 0..bytes.len() {
            assert!(decode(&bytes[..len]).is_err(), "accepted {len}");
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(matches!(
            decode(&bad),
            Err(AppearanceError::InvalidLength { .. })
        ));
        for at in [4, 6, 8] {
            let mut bad = bytes.clone();
            bad[at..at + 2].copy_from_slice(&u16::MAX.to_le_bytes());
            assert!(matches!(
                decode(&bad),
                Err(AppearanceError::InvalidLength { .. })
            ));
        }
        let mut bad = bytes;
        bad[10] = 1;
        assert_eq!(decode(&bad), Err(AppearanceError::ReservedNotZero(1)));
    }

    #[test]
    fn refuses_count_overflow_instead_of_wrapping() {
        let value = Appearance {
            parts: vec![
                PartChange {
                    part_index: 0,
                    gfx_obj_id: 0
                };
                65536
            ],
            ..Appearance::default()
        };
        assert_eq!(
            encode(&value),
            Err(AppearanceError::TooManyOperations("part"))
        );
    }
}
