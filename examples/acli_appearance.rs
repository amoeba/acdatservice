//! Synthetic, offline demo: `cargo run --example acli_appearance` prints ACLI hex.
//! No world database, R2, network or production data is used.
use acdatservice::{
    appearance::{Appearance, PaletteChange, PartChange, TextureChange},
    landblock_instances::{self as acli, LandblockInstance, LandblockInstances, TextureOverride},
};

fn main() {
    let first = LandblockInstance {
        setup_id: 0x0200_0001,
        origin: [1.0, 2.0, 3.0],
        orientation: [1.0, 0.0, 0.0, 0.0],
        scale: 1.0,
        palette_id: 0,
        palette_offset: 0,
        palette_length: 0,
        weenie_type: 10,
        flags: 0,
        motion_table_id: 0x0900_0001,
        physics_script_id: 0,
        guid: 0x8000_0001,
        texture_overrides: vec![TextureOverride {
            part_index: 2,
            texture_id: 0x0500_0003,
        }],
        appearance: Some(Appearance {
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
        }),
    };
    let mut absent = first.clone();
    absent.guid += 1;
    absent.appearance = None;
    absent.texture_overrides.clear();
    let mut empty = absent.clone();
    empty.guid += 1;
    empty.appearance = Some(Appearance::default());
    let value = LandblockInstances {
        landblock: 0x0125,
        instances: vec![first, absent, empty],
    };
    let bytes = acli::encode(&value);
    assert_eq!(acli::decode(&bytes).unwrap(), value);
    assert_eq!(value.encoded_len(), bytes.len());
    eprintln!(
        "ACLI appearance demo: {} records, {} bytes, featureBits={:#x}; roundtrip OK",
        value.instances.len(),
        bytes.len(),
        value.feature_bits()
    );
    for byte in bytes {
        print!("{byte:02x}");
    }
    println!();
}
