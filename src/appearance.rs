//! Resolved ACE ObjDesc operations carried by the ACLI appearance feature.
//!
//! Operations retain authored/layer order (later changes win). Palette ranges
//! are in blocks of eight colors. No renderer or database resolution happens here.

use std::{error::Error, fmt};

pub const HEADER_LEN: usize = 12;
pub const PART_LEN: usize = 6;
pub const TEXTURE_LEN: usize = 10;
pub const PALETTE_LEN: usize = 8;
pub const ATTACHMENT_LEN: usize = 16;
pub const MAX_ATTACHMENT_DEPTH: usize = 4;
pub const MAX_ATTACHMENT_NODES: usize = 32;

/// Held child Setup, resolved independently from its parent's appearance.
/// Owned Rust values cannot contain cyclic Appearance graphs.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldAttachment {
    pub setup_id: u32,
    pub parent_location: u16,
    pub placement_id: u16,
    pub scale: f32,
    pub appearance: Option<Appearance>,
}

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Appearance {
    /// Base palette (0x04) DAT id; zero means absent.
    pub palette_base_id: u32,
    pub parts: Vec<PartChange>,
    pub textures: Vec<TextureChange>,
    pub palettes: Vec<PaletteChange>,
    /// Empty is the canonical representation of absent optional attachments.
    pub attachments: Vec<HeldAttachment>,
}

impl Appearance {
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + self.parts.len() * PART_LEN
            + self.textures.len() * TEXTURE_LEN
            + self.palettes.len() * PALETTE_LEN
            + self
                .attachments
                .iter()
                .map(|child| {
                    ATTACHMENT_LEN + child.appearance.as_ref().map_or(0, Appearance::encoded_len)
                })
                .sum::<usize>()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppearanceError {
    TooShort { actual: usize },
    ReservedNotZero(u16),
    InvalidLength { expected: usize, actual: usize },
    TooManyOperations(&'static str),
    AttachmentsDisabled,
    AttachmentDepth,
    AttachmentNodes,
    InvalidAttachment(&'static str),
    TruncatedAttachment,
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
            Self::AttachmentsDisabled => write!(f, "attachments require feature mask 3"),
            Self::AttachmentDepth => write!(f, "attachment depth exceeds {MAX_ATTACHMENT_DEPTH}"),
            Self::AttachmentNodes => write!(f, "attachment nodes exceed {MAX_ATTACHMENT_NODES}"),
            Self::InvalidAttachment(field) => write!(f, "invalid attachment {field}"),
            Self::TruncatedAttachment => {
                write!(f, "truncated attachment header or child appearance")
            }
        }
    }
}

impl Error for AppearanceError {}

/// Standalone codec accepts attachments, matching TypeScript. ACLI always passes
/// the explicit global header mode via `encode_with_attachments`.
pub fn encode(value: &Appearance) -> Result<Vec<u8>, AppearanceError> {
    encode_with_attachments(value, true)
}

pub fn encode_with_attachments(
    value: &Appearance,
    enabled: bool,
) -> Result<Vec<u8>, AppearanceError> {
    encode_at(value, enabled, 0, &mut 0)
}

fn check_budget(count: usize, depth: usize, nodes: &mut usize) -> Result<(), AppearanceError> {
    if count != 0 && depth >= MAX_ATTACHMENT_DEPTH {
        return Err(AppearanceError::AttachmentDepth);
    }
    if count > MAX_ATTACHMENT_NODES - *nodes {
        return Err(AppearanceError::AttachmentNodes);
    }
    *nodes += count;
    Ok(())
}

fn check_attachment(
    setup: u32,
    parent: u16,
    placement: u16,
    scale: f32,
) -> Result<(), AppearanceError> {
    if setup >> 24 != 0x02 {
        return Err(AppearanceError::InvalidAttachment("setup id"));
    }
    if !matches!(parent, 1 | 2 | 3 | 8) {
        return Err(AppearanceError::InvalidAttachment("parent location"));
    }
    if !matches!(placement, 1 | 2 | 3 | 6) {
        return Err(AppearanceError::InvalidAttachment("placement id"));
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(AppearanceError::InvalidAttachment("scale"));
    }
    Ok(())
}

fn encode_at(
    value: &Appearance,
    enabled: bool,
    depth: usize,
    nodes: &mut usize,
) -> Result<Vec<u8>, AppearanceError> {
    if !enabled && !value.attachments.is_empty() {
        return Err(AppearanceError::AttachmentsDisabled);
    }
    check_budget(value.attachments.len(), depth, nodes)?;
    let parts =
        u16::try_from(value.parts.len()).map_err(|_| AppearanceError::TooManyOperations("part"))?;
    let textures = u16::try_from(value.textures.len())
        .map_err(|_| AppearanceError::TooManyOperations("texture"))?;
    let palettes = u16::try_from(value.palettes.len())
        .map_err(|_| AppearanceError::TooManyOperations("palette"))?;
    // Do not recursively size an unchecked in-memory graph before depth validation.
    let mut out = Vec::new();
    out.extend_from_slice(&value.palette_base_id.to_le_bytes());
    out.extend_from_slice(&parts.to_le_bytes());
    out.extend_from_slice(&textures.to_le_bytes());
    out.extend_from_slice(&palettes.to_le_bytes());
    out.extend_from_slice(&(value.attachments.len() as u16).to_le_bytes());
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
    for child in &value.attachments {
        check_attachment(
            child.setup_id,
            child.parent_location,
            child.placement_id,
            child.scale,
        )?;
        let bytes = child
            .appearance
            .as_ref()
            .map(|appearance| encode_at(appearance, enabled, depth + 1, nodes))
            .transpose()?
            .unwrap_or_default();
        let size = u32::try_from(bytes.len())
            .map_err(|_| AppearanceError::InvalidAttachment("appearance length"))?;
        out.extend_from_slice(&child.setup_id.to_le_bytes());
        out.extend_from_slice(&child.parent_location.to_le_bytes());
        out.extend_from_slice(&child.placement_id.to_le_bytes());
        out.extend_from_slice(&child.scale.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Appearance, AppearanceError> {
    decode_with_attachments(bytes, true)
}

pub fn decode_with_attachments(bytes: &[u8], enabled: bool) -> Result<Appearance, AppearanceError> {
    decode_at(bytes, enabled, 0, &mut 0)
}

fn decode_at(
    bytes: &[u8],
    enabled: bool,
    depth: usize,
    nodes: &mut usize,
) -> Result<Appearance, AppearanceError> {
    if bytes.len() < HEADER_LEN {
        return Err(AppearanceError::TooShort {
            actual: bytes.len(),
        });
    }
    let parts = read_u16(bytes, 4) as usize;
    let textures = read_u16(bytes, 6) as usize;
    let palettes = read_u16(bytes, 8) as usize;
    let reserved = read_u16(bytes, 10);
    if !enabled && reserved != 0 {
        return Err(AppearanceError::ReservedNotZero(reserved));
    }
    let expected = HEADER_LEN + parts * PART_LEN + textures * TEXTURE_LEN + palettes * PALETTE_LEN;
    if expected > bytes.len() || (reserved == 0 && expected != bytes.len()) {
        return Err(AppearanceError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    let attachments = reserved as usize;
    check_budget(attachments, depth, nodes)?;
    if attachments * ATTACHMENT_LEN > bytes.len() - expected {
        return Err(AppearanceError::TruncatedAttachment);
    }
    // Validate operation bounds and attachment budget before allocating from counts.
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
    for _ in 0..attachments {
        if bytes.len() - at < ATTACHMENT_LEN {
            return Err(AppearanceError::TruncatedAttachment);
        }
        let setup_id = read_u32(bytes, at);
        let parent_location = read_u16(bytes, at + 4);
        let placement_id = read_u16(bytes, at + 6);
        let scale = f32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap());
        check_attachment(setup_id, parent_location, placement_id, scale)?;
        let size = read_u32(bytes, at + 12) as usize;
        at += ATTACHMENT_LEN;
        if size > bytes.len() - at {
            return Err(AppearanceError::TruncatedAttachment);
        }
        let appearance = if size == 0 {
            None
        } else {
            Some(decode_at(&bytes[at..at + size], enabled, depth + 1, nodes)?)
        };
        at += size;
        value.attachments.push(HeldAttachment {
            setup_id,
            parent_location,
            placement_id,
            scale,
            appearance,
        });
    }
    if at != bytes.len() {
        return Err(AppearanceError::InvalidLength {
            expected: at,
            actual: bytes.len(),
        });
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
            ..Appearance::default()
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
        assert_eq!(
            decode_with_attachments(&bad, false),
            Err(AppearanceError::ReservedNotZero(1))
        );
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
