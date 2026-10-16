//! ACLB per-landblock bundle format (version 1).
//!
//! A bundle packs the DAT files that belong to a single landblock into one
//! self-describing blob. Two kinds exist:
//!
//! - [`BundleKind::Landblock`]: terrain (`…FFFF`) and, when present,
//!   LandblockInfo (`…FFFE`).
//! - [`BundleKind::Envcell`]: every EnvCell (`…01xx`–`…FExx`) owned by the
//!   block.
//!
//! The byte-level layout is specified in `docs/bundle-format.md`. This module
//! is the single Rust implementation; it is pure (no I/O) so the Worker and
//! the CLI share it.

use std::error::Error;
use std::fmt;

/// File magic: ASCII `ACLB` as little-endian bytes.
pub const MAGIC: u32 = u32::from_le_bytes(*b"ACLB");
/// Format version written by this crate.
pub const VERSION: u8 = 1;
/// Fixed header size in bytes.
pub const HEADER_LEN: usize = 16;
/// Fixed entry size in bytes: `u32 id` + `u32 offset` + `u32 length`.
pub const ENTRY_LEN: usize = 12;

/// Which DAT files a bundle contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BundleKind {
    /// Terrain (`…FFFF`) and optionally LandblockInfo (`…FFFE`).
    Landblock = 1,
    /// Every EnvCell owned by the landblock.
    Envcell = 2,
}

impl BundleKind {
    /// Parse the one-byte kind field. Returns `None` for unknown values.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Landblock),
            2 => Some(Self::Envcell),
            _ => None,
        }
    }

    /// The one-byte kind field written to disk.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// One bundled DAT file: its full file id and raw bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleEntry {
    pub id: u32,
    pub data: Vec<u8>,
}

/// A decoded or to-be-encoded bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    pub kind: BundleKind,
    /// High word shared by every entry id (`id >> 16`).
    pub landblock: u16,
    /// Entries in ascending id order.
    pub entries: Vec<BundleEntry>,
}

impl Bundle {
    /// Borrow the payload for `id`, if this bundle contains it.
    pub fn entry(&self, id: u32) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.data.as_slice())
    }

    /// Total encoded size in bytes.
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + self.entries.len() * ENTRY_LEN
            + self
                .entries
                .iter()
                .map(|entry| entry.data.len())
                .sum::<usize>()
    }
}

/// Everything that can go wrong encoding or decoding a bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    /// Fewer bytes than the fixed header requires.
    TooShort { needed: usize, actual: usize },
    /// The first four bytes were not [`MAGIC`].
    BadMagic(u32),
    /// The version byte is not a version this crate understands.
    UnsupportedVersion(u8),
    /// The kind byte is not a known [`BundleKind`].
    UnknownKind(u8),
    /// A reserved field was not zero.
    ReservedNotZero(u16),
    /// The landblock field did not fit in 16 bits.
    LandblockOutOfRange(u32),
    /// `count * ENTRY_LEN` (or a payload end) overflowed `usize`.
    SizeOverflow,
    /// The entry table runs past the end of the input.
    TruncatedTable { count: u32, available: usize },
    /// An entry's payload runs past the end of the input.
    TruncatedPayload {
        id: u32,
        needed: usize,
        available: usize,
    },
    /// An entry's high word does not match the bundle's landblock.
    IdOutsideLandblock { id: u32, landblock: u16 },
    /// An entry's payload offset points into the header or entry table.
    PayloadBeforeTable {
        id: u32,
        offset: usize,
        payload_start: usize,
    },
    /// Payloads contain a gap or overlap at the given offset.
    PayloadContiguity { at: usize },
    /// There were bytes left over after the last payload.
    TrailingBytes { extra: usize },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundleError::TooShort { needed, actual } => {
                write!(f, "bundle too short: need {needed} bytes, got {actual}")
            }
            BundleError::BadMagic(magic) => {
                write!(
                    f,
                    "bad bundle magic \"{}\", expected \"ACLB\"",
                    magic_tag(*magic)
                )
            }
            BundleError::UnsupportedVersion(version) => {
                write!(f, "unsupported bundle version {version}")
            }
            BundleError::UnknownKind(kind) => write!(f, "unknown bundle kind {kind}"),
            BundleError::ReservedNotZero(value) => {
                write!(f, "reserved header field must be zero, got {value}")
            }
            BundleError::LandblockOutOfRange(value) => {
                write!(f, "landblock {value} does not fit in 16 bits")
            }
            BundleError::SizeOverflow => write!(f, "bundle size overflowed usize"),
            BundleError::TruncatedTable { count, available } => write!(
                f,
                "entry table for {count} entries does not fit in {available} available bytes"
            ),
            BundleError::TruncatedPayload {
                id,
                needed,
                available,
            } => write!(
                f,
                "payload for entry 0x{id:08X} needs {needed} bytes, only {available} available"
            ),
            BundleError::IdOutsideLandblock { id, landblock } => {
                write!(f, "entry 0x{id:08X} is outside landblock 0x{landblock:04X}")
            }
            BundleError::PayloadBeforeTable {
                id,
                offset,
                payload_start,
            } => write!(
                f,
                "payload for entry 0x{id:08X} starts at {offset}, before the payload region ({payload_start})"
            ),
            BundleError::PayloadContiguity { at } => {
                write!(f, "payloads are not contiguous at byte {at}")
            }
            BundleError::TrailingBytes { extra } => {
                write!(f, "{extra} trailing bytes after the last payload")
            }
        }
    }
}

impl Error for BundleError {}

/// Encode a bundle. Payloads are written in the order given and their
/// absolute offsets are recorded in the entry table so readers can seek
/// directly. Callers are expected to sort entries by id first.
pub fn encode(bundle: &Bundle) -> Vec<u8> {
    let count = bundle.entries.len();
    let mut out = Vec::with_capacity(bundle.encoded_len());

    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(VERSION);
    out.push(bundle.kind.as_u8());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(bundle.landblock as u32).to_le_bytes());
    out.extend_from_slice(&(count as u32).to_le_bytes());

    let mut payload_at = HEADER_LEN + count * ENTRY_LEN;
    for entry in &bundle.entries {
        out.extend_from_slice(&entry.id.to_le_bytes());
        out.extend_from_slice(&(payload_at as u32).to_le_bytes());
        out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        payload_at += entry.data.len();
    }
    for entry in &bundle.entries {
        out.extend_from_slice(&entry.data);
    }

    out
}

/// Decode a bundle, validating every field and bound.
///
/// Payload offsets are absolute file offsets. They are validated to tile the
/// payload region exactly, so a reader can trust them for random access.
pub fn decode(bytes: &[u8]) -> Result<Bundle, BundleError> {
    if bytes.len() < HEADER_LEN {
        return Err(BundleError::TooShort {
            needed: HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let magic = read_u32(bytes, 0);
    if magic != MAGIC {
        return Err(BundleError::BadMagic(magic));
    }

    let version = bytes[4];
    if version != VERSION {
        return Err(BundleError::UnsupportedVersion(version));
    }

    let kind = BundleKind::from_u8(bytes[5]).ok_or(BundleError::UnknownKind(bytes[5]))?;

    let reserved = read_u16(bytes, 6);
    if reserved != 0 {
        return Err(BundleError::ReservedNotZero(reserved));
    }

    let raw_landblock = read_u32(bytes, 8);
    let landblock = u16::try_from(raw_landblock)
        .map_err(|_| BundleError::LandblockOutOfRange(raw_landblock))?;

    let count = read_u32(bytes, 12) as usize;

    let table_len = count
        .checked_mul(ENTRY_LEN)
        .ok_or(BundleError::SizeOverflow)?;
    let payload_start = HEADER_LEN
        .checked_add(table_len)
        .ok_or(BundleError::SizeOverflow)?;
    if bytes.len() < payload_start {
        return Err(BundleError::TruncatedTable {
            count: count as u32,
            available: bytes.len().saturating_sub(HEADER_LEN),
        });
    }

    // Read and bound-check the table before slicing any payload.
    let mut table = Vec::with_capacity(count);
    for index in 0..count {
        let at = HEADER_LEN + index * ENTRY_LEN;
        let id = read_u32(bytes, at);
        let offset = read_u32(bytes, at + 4) as usize;
        let length = read_u32(bytes, at + 8) as usize;

        if (id >> 16) as u16 != landblock {
            return Err(BundleError::IdOutsideLandblock { id, landblock });
        }
        if offset < payload_start {
            return Err(BundleError::PayloadBeforeTable {
                id,
                offset,
                payload_start,
            });
        }
        let end = offset
            .checked_add(length)
            .ok_or(BundleError::SizeOverflow)?;
        if end > bytes.len() {
            return Err(BundleError::TruncatedPayload {
                id,
                needed: length,
                available: bytes.len().saturating_sub(offset),
            });
        }
        table.push((id, offset, length));
    }

    // Payloads must exactly tile the region after the table: no gaps, no
    // overlaps, no trailing bytes. Checked order-independently so the table
    // order is not load-bearing.
    let mut ranges: Vec<(usize, usize)> = table
        .iter()
        .map(|&(_, offset, length)| (offset, offset + length))
        .collect();
    ranges.sort_unstable();

    if count == 0 {
        if bytes.len() != payload_start {
            return Err(BundleError::TrailingBytes {
                extra: bytes.len() - payload_start,
            });
        }
    } else {
        if ranges[0].0 != payload_start {
            return Err(BundleError::PayloadContiguity { at: payload_start });
        }
        for pair in ranges.windows(2) {
            if pair[0].1 != pair[1].0 {
                return Err(BundleError::PayloadContiguity { at: pair[0].1 });
            }
        }
        let end = ranges[ranges.len() - 1].1;
        if end < bytes.len() {
            return Err(BundleError::TrailingBytes {
                extra: bytes.len() - end,
            });
        }
    }

    let mut entries = Vec::with_capacity(count);
    for (id, offset, length) in table {
        entries.push(BundleEntry {
            id,
            data: bytes[offset..offset + length].to_vec(),
        });
    }

    Ok(Bundle {
        kind,
        landblock,
        entries,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u32, data: &[u8]) -> BundleEntry {
        BundleEntry {
            id,
            data: data.to_vec(),
        }
    }

    fn landblock_bundle() -> Bundle {
        Bundle {
            kind: BundleKind::Landblock,
            landblock: 0xB34A,
            entries: vec![
                entry(0xB34A_FFFE, &[1, 2, 3, 4]),
                entry(0xB34A_FFFF, &[0xAA; 252]),
            ],
        }
    }

    fn envcell_bundle() -> Bundle {
        Bundle {
            kind: BundleKind::Envcell,
            landblock: 0xB34A,
            entries: vec![
                entry(0xB34A_0100, &[9, 8, 7]),
                entry(0xB34A_0101, &[6, 5]),
                entry(0xB34A_0102, &[]),
            ],
        }
    }

    #[test]
    fn roundtrips_landblock_bundle() {
        let bundle = landblock_bundle();
        let decoded = decode(&encode(&bundle)).unwrap();
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn roundtrips_envcell_bundle() {
        let bundle = envcell_bundle();
        let decoded = decode(&encode(&bundle)).unwrap();
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn roundtrips_empty_bundle() {
        let bundle = Bundle {
            kind: BundleKind::Envcell,
            landblock: 0x0000,
            entries: Vec::new(),
        };
        let encoded = encode(&bundle);
        assert_eq!(encoded.len(), HEADER_LEN);
        assert_eq!(decode(&encoded).unwrap(), bundle);
    }

    #[test]
    fn header_layout_is_little_endian() {
        let encoded = encode(&landblock_bundle());

        assert_eq!(&encoded[0..4], b"ACLB");
        assert_eq!(encoded[4], VERSION);
        assert_eq!(encoded[5], BundleKind::Landblock.as_u8());
        assert_eq!(&encoded[6..8], &[0, 0]);
        assert_eq!(&encoded[8..12], &0xB34Au32.to_le_bytes());
        assert_eq!(&encoded[12..16], &2u32.to_le_bytes());

        // First entry: id, absolute offset, then length.
        let payload_start = (HEADER_LEN + 2 * ENTRY_LEN) as u32;
        assert_eq!(&encoded[16..20], &0xB34A_FFFEu32.to_le_bytes());
        assert_eq!(&encoded[20..24], &payload_start.to_le_bytes());
        assert_eq!(&encoded[24..28], &4u32.to_le_bytes());
    }

    /// The exact bytes from the worked example in `docs/bundle-format.md`.
    #[test]
    fn worked_example_matches_docs() {
        let bundle = Bundle {
            kind: BundleKind::Envcell,
            landblock: 0x00AE,
            entries: vec![
                entry(0x00AE_0100, &[0xDE, 0xAD]),
                entry(0x00AE_0101, &[0xBE, 0xEF, 0x00]),
            ],
        };

        #[rustfmt::skip]
        let expected: [u8; 45] = [
            0x41, 0x43, 0x4C, 0x42, // magic "ACLB"
            0x01,                   // version
            0x02,                   // kind = envcell
            0x00, 0x00,             // reserved
            0xAE, 0x00, 0x00, 0x00, // landblock = 0x00AE
            0x02, 0x00, 0x00, 0x00, // count = 2
            0x00, 0x01, 0xAE, 0x00, // entry 0 id = 0x00AE0100
            0x28, 0x00, 0x00, 0x00, // entry 0 offset = 40
            0x02, 0x00, 0x00, 0x00, // entry 0 length = 2
            0x01, 0x01, 0xAE, 0x00, // entry 1 id = 0x00AE0101
            0x2A, 0x00, 0x00, 0x00, // entry 1 offset = 42
            0x03, 0x00, 0x00, 0x00, // entry 1 length = 3
            0xDE, 0xAD,             // entry 0 payload
            0xBE, 0xEF, 0x00,       // entry 1 payload
        ];

        assert_eq!(encode(&bundle), expected);
        assert_eq!(decode(&expected).unwrap(), bundle);
    }

    #[test]
    fn entry_lookup_returns_payload() {
        let bundle = envcell_bundle();
        assert_eq!(bundle.entry(0xB34A_0101), Some(&[6, 5][..]));
        assert_eq!(bundle.entry(0xB34A_9999), None);
    }

    #[test]
    fn encoded_len_matches_encode() {
        for bundle in [landblock_bundle(), envcell_bundle()] {
            assert_eq!(bundle.encoded_len(), encode(&bundle).len());
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let mut encoded = encode(&landblock_bundle());
        encoded[0] = b'X';
        assert!(matches!(decode(&encoded), Err(BundleError::BadMagic(_))));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut encoded = encode(&landblock_bundle());
        encoded[4] = 0xFF;
        assert_eq!(decode(&encoded), Err(BundleError::UnsupportedVersion(0xFF)));
    }

    #[test]
    fn rejects_unknown_kind() {
        let mut encoded = encode(&landblock_bundle());
        encoded[5] = 9;
        assert_eq!(decode(&encoded), Err(BundleError::UnknownKind(9)));
    }

    #[test]
    fn rejects_nonzero_reserved() {
        let mut encoded = encode(&landblock_bundle());
        encoded[6] = 1;
        assert_eq!(decode(&encoded), Err(BundleError::ReservedNotZero(1)));
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            decode(&[0u8; 8]),
            Err(BundleError::TooShort {
                needed: HEADER_LEN,
                actual: 8
            })
        );
    }

    #[test]
    fn rejects_truncated_table() {
        let encoded = encode(&envcell_bundle());
        // Keep the header but drop the entry table.
        assert_eq!(
            decode(&encoded[..HEADER_LEN]),
            Err(BundleError::TruncatedTable {
                count: 3,
                available: 0
            })
        );
    }

    #[test]
    fn rejects_truncated_payload() {
        let encoded = encode(&envcell_bundle());
        let truncated = &encoded[..encoded.len() - 1];
        assert!(matches!(
            decode(truncated),
            Err(BundleError::TruncatedPayload { .. })
        ));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut encoded = encode(&envcell_bundle());
        encoded.push(0);
        assert_eq!(
            decode(&encoded),
            Err(BundleError::TrailingBytes { extra: 1 })
        );
    }

    #[test]
    fn rejects_id_outside_landblock() {
        let mut encoded = encode(&envcell_bundle());
        // Point the first entry at a different landblock.
        encoded[16..20].copy_from_slice(&0xB34B_0100u32.to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(BundleError::IdOutsideLandblock {
                id: 0xB34B_0100,
                landblock: 0xB34A
            })
        );
    }

    #[test]
    fn rejects_landblock_that_does_not_fit_u16() {
        let mut encoded = encode(&envcell_bundle());
        encoded[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(BundleError::LandblockOutOfRange(0x0001_0000))
        );
    }

    #[test]
    fn rejects_offset_inside_the_table() {
        let mut encoded = encode(&envcell_bundle());
        // Entry 0's offset is at bytes 20..24.
        encoded[20..24].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            decode(&encoded),
            Err(BundleError::PayloadBeforeTable {
                id: 0xB34A_0100,
                ..
            })
        ));
    }

    #[test]
    fn rejects_payload_gap() {
        let mut encoded = encode(&envcell_bundle());
        // Shift entry 0's offset one byte later, leaving a one-byte gap.
        let payload_start = (HEADER_LEN + 3 * ENTRY_LEN) as u32;
        encoded[20..24].copy_from_slice(&(payload_start + 1).to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(BundleError::PayloadContiguity {
                at: payload_start as usize
            })
        );
    }

    #[test]
    fn error_messages_are_descriptive() {
        assert!(BundleError::BadMagic(0).to_string().contains("magic"));
        assert_eq!(magic_tag(MAGIC), "ACLB");
        assert!(BundleError::UnknownKind(9).to_string().contains("kind"));
        assert!(BundleError::TrailingBytes { extra: 3 }
            .to_string()
            .contains('3'));
    }
}
