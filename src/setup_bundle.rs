//! ACSP per-setup bundle format (version 1).
//!
//! A setup bundle packs the portal DAT files that make up a single Setup's
//! closure into one self-describing blob. A Setup references 15–167 portal
//! files (GfxObjs, surfaces, palettes, textures, …), so one object per setup
//! turns many small reads into a single deterministic, immutable cache entry.
//!
//! Unlike [`crate::bundle`], an entry may be *known missing*: the generator
//! knows the id belongs to the closure but the DAT has no such file. A missing
//! entry is recorded with the [`MISSING`] length marker and no payload, and
//! decodes to [`SetupEntry::data`] `== None`.
//!
//! The byte-level layout is specified in `docs/setup-bundle-format.md`. This
//! module is the single Rust implementation; it is pure (no I/O) so the Worker
//! and the CLI share it.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

/// File magic: ASCII `ACSP` as little-endian bytes.
pub const MAGIC: u32 = u32::from_le_bytes(*b"ACSP");
/// Format version written by this crate.
pub const VERSION: u8 = 1;
/// Fixed header size in bytes.
pub const HEADER_LEN: usize = 16;
/// Fixed entry size in bytes: `u32 id` + `u32 offset` + `u32 length`.
pub const ENTRY_LEN: usize = 12;
/// Length sentinel marking an id that is known to be absent.
pub const MISSING: u32 = 0xFFFF_FFFF;

/// The one-byte kind field for setup bundles.
const SETUP_KIND: u8 = 3;

/// One Setup closure member: its portal DAT file id and, unless
/// [`MISSING`], its raw bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupEntry {
    pub id: u32,
    /// `None` marks a known-missing id: the closure references it but the DAT
    /// does not contain it.
    pub data: Option<Vec<u8>>,
}

/// A decoded or to-be-encoded setup bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupBundle {
    pub setup_id: u32,
    /// Entries; callers SHOULD keep these in ascending id order.
    pub entries: Vec<SetupEntry>,
}

impl SetupBundle {
    /// Borrow the entry for `id`, if this bundle contains it.
    pub fn entry(&self, id: u32) -> Option<&SetupEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Look up `id`'s payload.
    ///
    /// - `None` — the id is absent from this bundle.
    /// - `Some(None)` — the id is a known-missing member.
    /// - `Some(Some(bytes))` — the id is present with a payload (which may be
    ///   empty).
    pub fn payload(&self, id: u32) -> Option<Option<&[u8]>> {
        self.entry(id).map(|entry| entry.data.as_deref())
    }

    /// Total encoded size in bytes.
    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + self.entries.len() * ENTRY_LEN
            + self
                .entries
                .iter()
                .filter_map(|entry| entry.data.as_ref())
                .map(|data| data.len())
                .sum::<usize>()
    }
}

/// Everything that can go wrong encoding or decoding a setup bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupBundleError {
    /// Fewer bytes than the fixed header requires.
    TooShort { needed: usize, actual: usize },
    /// The first four bytes were not [`MAGIC`].
    BadMagic(u32),
    /// The version byte is not a version this crate understands.
    UnsupportedVersion(u8),
    /// The kind byte is not the setup kind.
    UnknownKind(u8),
    /// A reserved field was not zero.
    ReservedNotZero(u16),
    /// `count * ENTRY_LEN` (or a payload end) overflowed `usize`.
    SizeOverflow,
    /// The entry table runs past the end of the input.
    TruncatedTable { count: u32, available: usize },
    /// A present payload runs past the end of the input.
    TruncatedPayload {
        id: u32,
        needed: usize,
        available: usize,
    },
    /// A present payload offset points into the header or entry table.
    PayloadBeforeTable { id: u32, offset: usize },
    /// Payload ranges contain a gap, an overlap, or trailing bytes.
    PayloadRangesDoNotTile,
    /// The same id appears more than once.
    DuplicateEntry { id: u32 },
}

impl fmt::Display for SetupBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupBundleError::TooShort { needed, actual } => {
                write!(
                    f,
                    "setup bundle too short: need {needed} bytes, got {actual}"
                )
            }
            SetupBundleError::BadMagic(magic) => {
                write!(
                    f,
                    "bad setup bundle magic \"{}\", expected \"ACSP\"",
                    magic_tag(*magic)
                )
            }
            SetupBundleError::UnsupportedVersion(version) => {
                write!(f, "unsupported setup bundle version {version}")
            }
            SetupBundleError::UnknownKind(kind) => write!(f, "unknown setup bundle kind {kind}"),
            SetupBundleError::ReservedNotZero(value) => {
                write!(f, "reserved header field must be zero, got {value}")
            }
            SetupBundleError::SizeOverflow => write!(f, "setup bundle size overflowed usize"),
            SetupBundleError::TruncatedTable { count, available } => write!(
                f,
                "entry table for {count} entries does not fit in {available} available bytes"
            ),
            SetupBundleError::TruncatedPayload {
                id,
                needed,
                available,
            } => write!(
                f,
                "payload for entry 0x{id:08X} needs {needed} bytes, only {available} available"
            ),
            SetupBundleError::PayloadBeforeTable { id, offset } => write!(
                f,
                "payload for entry 0x{id:08X} starts at {offset}, before the payload region"
            ),
            SetupBundleError::PayloadRangesDoNotTile => {
                write!(f, "payload ranges do not tile the payload region exactly")
            }
            SetupBundleError::DuplicateEntry { id } => {
                write!(f, "duplicate entry id 0x{id:08X}")
            }
        }
    }
}

impl Error for SetupBundleError {}

/// Encode a setup bundle. Payloads are written in the order given and their
/// absolute offsets are recorded in the entry table. Entries with
/// `data == None` are written as known-missing markers and contribute no
/// payload. Callers are expected to sort entries by id first.
pub fn encode(bundle: &SetupBundle) -> Vec<u8> {
    let count = bundle.entries.len();
    let mut out = Vec::with_capacity(bundle.encoded_len());

    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(VERSION);
    out.push(SETUP_KIND);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&bundle.setup_id.to_le_bytes());
    out.extend_from_slice(&(count as u32).to_le_bytes());

    let mut payload_at = HEADER_LEN + count * ENTRY_LEN;
    for entry in &bundle.entries {
        out.extend_from_slice(&entry.id.to_le_bytes());
        match &entry.data {
            None => {
                out.extend_from_slice(&0u32.to_le_bytes());
                out.extend_from_slice(&MISSING.to_le_bytes());
            }
            Some(data) => {
                out.extend_from_slice(&(payload_at as u32).to_le_bytes());
                out.extend_from_slice(&(data.len() as u32).to_le_bytes());
                payload_at += data.len();
            }
        }
    }
    for entry in &bundle.entries {
        if let Some(data) = &entry.data {
            out.extend_from_slice(data);
        }
    }

    out
}

/// Decode a setup bundle, validating every field and bound.
///
/// Payload offsets are absolute file offsets. Present payloads are validated
/// to tile the region after the entry table exactly, so a reader can trust
/// them for random access. Known-missing entries carry no payload and
/// therefore contribute no range.
pub fn decode(bytes: &[u8]) -> Result<SetupBundle, SetupBundleError> {
    if bytes.len() < HEADER_LEN {
        return Err(SetupBundleError::TooShort {
            needed: HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let magic = read_u32(bytes, 0);
    if magic != MAGIC {
        return Err(SetupBundleError::BadMagic(magic));
    }

    let version = bytes[4];
    if version != VERSION {
        return Err(SetupBundleError::UnsupportedVersion(version));
    }

    if bytes[5] != SETUP_KIND {
        return Err(SetupBundleError::UnknownKind(bytes[5]));
    }

    let reserved = read_u16(bytes, 6);
    if reserved != 0 {
        return Err(SetupBundleError::ReservedNotZero(reserved));
    }

    let setup_id = read_u32(bytes, 8);
    let count = read_u32(bytes, 12) as usize;

    let table_len = count
        .checked_mul(ENTRY_LEN)
        .ok_or(SetupBundleError::SizeOverflow)?;
    let payload_start = HEADER_LEN
        .checked_add(table_len)
        .ok_or(SetupBundleError::SizeOverflow)?;
    if bytes.len() < payload_start {
        return Err(SetupBundleError::TruncatedTable {
            count: count as u32,
            available: bytes.len().saturating_sub(HEADER_LEN),
        });
    }

    // Read and bound-check the table before slicing any payload.
    let mut seen = HashSet::with_capacity(count);
    let mut table = Vec::with_capacity(count);
    for index in 0..count {
        let at = HEADER_LEN + index * ENTRY_LEN;
        let id = read_u32(bytes, at);
        let offset = read_u32(bytes, at + 4) as usize;
        let length = read_u32(bytes, at + 8);

        if !seen.insert(id) {
            return Err(SetupBundleError::DuplicateEntry { id });
        }

        if length == MISSING {
            // Known-missing: the offset field is mandated to be 0 on write but
            // is otherwise ignored, and the entry contributes no payload range.
            table.push((id, None));
            continue;
        }

        if offset < payload_start {
            return Err(SetupBundleError::PayloadBeforeTable { id, offset });
        }
        let end = offset
            .checked_add(length as usize)
            .ok_or(SetupBundleError::SizeOverflow)?;
        if end > bytes.len() {
            return Err(SetupBundleError::TruncatedPayload {
                id,
                needed: length as usize,
                available: bytes.len().saturating_sub(offset),
            });
        }
        table.push((id, Some((offset, end))));
    }

    // Present payloads must exactly tile the region after the table: no gaps,
    // no overlaps, no trailing bytes. Checked order-independently so the table
    // order is not load-bearing. Known-missing entries add no range.
    let mut ranges: Vec<(usize, usize)> = table.iter().filter_map(|&(_, range)| range).collect();
    ranges.sort_unstable();

    if ranges.is_empty() {
        if bytes.len() != payload_start {
            return Err(SetupBundleError::PayloadRangesDoNotTile);
        }
    } else {
        if ranges[0].0 != payload_start {
            return Err(SetupBundleError::PayloadRangesDoNotTile);
        }
        for pair in ranges.windows(2) {
            if pair[0].1 != pair[1].0 {
                return Err(SetupBundleError::PayloadRangesDoNotTile);
            }
        }
        if ranges[ranges.len() - 1].1 != bytes.len() {
            return Err(SetupBundleError::PayloadRangesDoNotTile);
        }
    }

    let mut entries = Vec::with_capacity(count);
    for (id, range) in table {
        let data = range.map(|(offset, end)| bytes[offset..end].to_vec());
        entries.push(SetupEntry { id, data });
    }

    Ok(SetupBundle { setup_id, entries })
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

    fn present(id: u32, data: &[u8]) -> SetupEntry {
        SetupEntry {
            id,
            data: Some(data.to_vec()),
        }
    }

    fn missing(id: u32) -> SetupEntry {
        SetupEntry { id, data: None }
    }

    fn sample_bundle() -> SetupBundle {
        SetupBundle {
            setup_id: 0x0000_002A,
            entries: vec![
                present(0x0200_0001, &[0xDE, 0xAD]),
                // Zero-length present payload is valid.
                present(0x0300_0001, &[]),
                missing(0x0600_0000),
            ],
        }
    }

    #[test]
    fn roundtrips_present_and_missing_entries() {
        let bundle = sample_bundle();
        let decoded = decode(&encode(&bundle)).unwrap();
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn roundtrips_empty_bundle() {
        let bundle = SetupBundle {
            setup_id: 0,
            entries: Vec::new(),
        };
        let encoded = encode(&bundle);
        assert_eq!(encoded.len(), HEADER_LEN);
        assert_eq!(decode(&encoded).unwrap(), bundle);
    }

    #[test]
    fn header_layout_is_little_endian() {
        let encoded = encode(&sample_bundle());

        assert_eq!(&encoded[0..4], b"ACSP");
        assert_eq!(encoded[4], VERSION);
        assert_eq!(encoded[5], SETUP_KIND);
        assert_eq!(&encoded[6..8], &[0, 0]);
        assert_eq!(&encoded[8..12], &0x0000_002Au32.to_le_bytes());
        assert_eq!(&encoded[12..16], &3u32.to_le_bytes());

        // First entry: id, absolute offset, then length.
        let payload_start = (HEADER_LEN + 3 * ENTRY_LEN) as u32;
        assert_eq!(&encoded[16..20], &0x0200_0001u32.to_le_bytes());
        assert_eq!(&encoded[20..24], &payload_start.to_le_bytes());
        assert_eq!(&encoded[24..28], &2u32.to_le_bytes());

        // Third entry is a known-missing marker with no payload.
        assert_eq!(&encoded[40..44], &0x0600_0000u32.to_le_bytes());
        assert_eq!(&encoded[44..48], &0u32.to_le_bytes());
        assert_eq!(&encoded[48..52], &MISSING.to_le_bytes());
    }

    /// The exact bytes from the worked example in `docs/setup-bundle-format.md`.
    #[test]
    fn worked_example_matches_docs() {
        let bundle = SetupBundle {
            setup_id: 0x0000_0001,
            entries: vec![present(0x0200_0001, &[0xDE, 0xAD]), missing(0x0600_0000)],
        };

        #[rustfmt::skip]
        let expected: [u8; 42] = [
            0x41, 0x43, 0x53, 0x50, // magic "ACSP"
            0x01,                   // version
            0x03,                   // kind = setup
            0x00, 0x00,             // reserved
            0x01, 0x00, 0x00, 0x00, // setup_id = 1
            0x02, 0x00, 0x00, 0x00, // count = 2
            0x01, 0x00, 0x00, 0x02, // entry 0 id = 0x02000001
            0x28, 0x00, 0x00, 0x00, // entry 0 offset = 40
            0x02, 0x00, 0x00, 0x00, // entry 0 length = 2
            0x00, 0x00, 0x00, 0x06, // entry 1 id = 0x06000000
            0x00, 0x00, 0x00, 0x00, // entry 1 offset = 0
            0xFF, 0xFF, 0xFF, 0xFF, // entry 1 length = MISSING
            0xDE, 0xAD,             // entry 0 payload
        ];

        assert_eq!(encode(&bundle), expected);
        assert_eq!(decode(&expected).unwrap(), bundle);
    }

    #[test]
    fn payload_lookup_distinguishes_absent_missing_and_present() {
        let bundle = sample_bundle();
        assert_eq!(bundle.payload(0x0200_0001), Some(Some(&[0xDE, 0xAD][..])));
        assert_eq!(bundle.payload(0x0300_0001), Some(Some(&[][..])));
        assert_eq!(bundle.payload(0x0600_0000), Some(None));
        assert_eq!(bundle.payload(0x9999_9999), None);
    }

    #[test]
    fn entry_lookup_returns_entry() {
        let bundle = sample_bundle();
        assert_eq!(bundle.entry(0x0200_0001).unwrap().id, 0x0200_0001);
        assert_eq!(bundle.entry(0x9999_9999), None);
    }

    #[test]
    fn encoded_len_matches_encode() {
        for bundle in [
            sample_bundle(),
            SetupBundle {
                setup_id: 1,
                entries: Vec::new(),
            },
        ] {
            assert_eq!(bundle.encoded_len(), encode(&bundle).len());
        }
    }

    // Invariant 1.
    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            decode(&[0u8; 8]),
            Err(SetupBundleError::TooShort {
                needed: HEADER_LEN,
                actual: 8
            })
        );
    }

    // Invariant 2.
    #[test]
    fn rejects_bad_magic() {
        let mut encoded = encode(&sample_bundle());
        encoded[0] = b'X';
        assert!(matches!(
            decode(&encoded),
            Err(SetupBundleError::BadMagic(_))
        ));
    }

    // Invariant 3.
    #[test]
    fn rejects_unsupported_version() {
        let mut encoded = encode(&sample_bundle());
        encoded[4] = 0xFF;
        assert_eq!(
            decode(&encoded),
            Err(SetupBundleError::UnsupportedVersion(0xFF))
        );
    }

    // Invariant 4.
    #[test]
    fn rejects_unknown_kind() {
        let mut encoded = encode(&sample_bundle());
        encoded[5] = 9;
        assert_eq!(decode(&encoded), Err(SetupBundleError::UnknownKind(9)));
    }

    // Invariant 5.
    #[test]
    fn rejects_nonzero_reserved() {
        let mut encoded = encode(&sample_bundle());
        encoded[6] = 1;
        assert_eq!(decode(&encoded), Err(SetupBundleError::ReservedNotZero(1)));
    }

    // Invariant 6.
    #[test]
    fn rejects_truncated_table() {
        let encoded = encode(&sample_bundle());
        // Keep the header but drop the entry table.
        assert_eq!(
            decode(&encoded[..HEADER_LEN]),
            Err(SetupBundleError::TruncatedTable {
                count: 3,
                available: 0
            })
        );
    }

    // Invariant 7.
    #[test]
    fn rejects_truncated_payload() {
        let encoded = encode(&sample_bundle());
        let truncated = &encoded[..encoded.len() - 1];
        assert!(matches!(
            decode(truncated),
            Err(SetupBundleError::TruncatedPayload { .. })
        ));
    }

    // Invariant 8.
    #[test]
    fn rejects_offset_inside_the_table() {
        let mut encoded = encode(&sample_bundle());
        // Entry 0's offset is at bytes 20..24.
        encoded[20..24].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(SetupBundleError::PayloadBeforeTable {
                id: 0x0200_0001,
                offset: 0,
            })
        );
    }

    // Invariant 9.
    #[test]
    fn rejects_payload_gap() {
        let mut encoded = encode(&sample_bundle());
        // Append a byte so a shifted payload can stay in-bounds.
        encoded.push(0);
        // Move the zero-length present entry (entry 1, offset at 32..36) one
        // byte past the cursor, leaving a one-byte gap after entry 0.
        let payload_end = (HEADER_LEN + 3 * ENTRY_LEN) as u32 + 2;
        encoded[32..36].copy_from_slice(&(payload_end + 1).to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(SetupBundleError::PayloadRangesDoNotTile)
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut encoded = encode(&sample_bundle());
        encoded.push(0);
        assert_eq!(
            decode(&encoded),
            Err(SetupBundleError::PayloadRangesDoNotTile)
        );
    }

    #[test]
    fn rejects_payload_overlap() {
        let bundle = SetupBundle {
            setup_id: 1,
            entries: vec![present(0x0200_0001, &[1, 2]), present(0x0200_0002, &[3, 4])],
        };
        let mut encoded = encode(&bundle);
        // Entry 1's offset lives at 16 + 12 + 4 = 32..36. Point it one byte back
        // into entry 0's payload so the two ranges overlap (and leave a trailing
        // byte), which must be rejected.
        let payload_start = (HEADER_LEN + 2 * ENTRY_LEN) as u32;
        encoded[32..36].copy_from_slice(&(payload_start + 1).to_le_bytes());
        assert_eq!(
            decode(&encoded),
            Err(SetupBundleError::PayloadRangesDoNotTile)
        );
    }

    #[test]
    fn all_missing_entries_are_valid_and_reject_trailing_bytes() {
        // Exercises the empty-ranges branch with count > 0 (no present payloads).
        let bundle = SetupBundle {
            setup_id: 1,
            entries: vec![missing(0x0200_0001), missing(0x0200_0002)],
        };
        let encoded = encode(&bundle);
        assert_eq!(encoded.len(), HEADER_LEN + 2 * ENTRY_LEN);
        assert_eq!(decode(&encoded).unwrap(), bundle);

        let mut with_trailing = encoded.clone();
        with_trailing.push(0);
        assert_eq!(
            decode(&with_trailing),
            Err(SetupBundleError::PayloadRangesDoNotTile)
        );
    }

    // Invariant 10.
    #[test]
    fn rejects_duplicate_ids() {
        let bundle = SetupBundle {
            setup_id: 1,
            entries: vec![present(0x0200_0001, &[]), present(0x0200_0001, &[])],
        };
        assert_eq!(
            decode(&encode(&bundle)),
            Err(SetupBundleError::DuplicateEntry { id: 0x0200_0001 })
        );
    }

    #[test]
    fn error_messages_are_descriptive() {
        assert!(SetupBundleError::BadMagic(0).to_string().contains("magic"));
        assert_eq!(magic_tag(MAGIC), "ACSP");
        assert!(SetupBundleError::UnknownKind(9)
            .to_string()
            .contains("kind"));
        assert!(SetupBundleError::DuplicateEntry { id: 2 }
            .to_string()
            .contains("0x00000002"));
    }
}
