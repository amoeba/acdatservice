use acdatservice::{
    bundle::{self, Bundle, BundleEntry, BundleKind},
    setup_bundle::{self, SetupBundle, SetupEntry},
};

#[test]
fn javascript_pack_fixtures_match_rust_encoders_and_decoders() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/service-packs.json")).unwrap();
    for value in cases {
        let hex = value["hex"].as_str().unwrap();
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let entries = value["entries"].as_array().unwrap();
        if value["format"] == "ACLB" {
            let expected = Bundle {
                kind: BundleKind::from_u8(value["kind"].as_u64().unwrap() as u8).unwrap(),
                landblock: value["landblock"].as_u64().unwrap() as u16,
                entries: entries
                    .iter()
                    .map(|entry| BundleEntry {
                        id: entry["id"].as_u64().unwrap() as u32,
                        data: entry["data"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|byte| byte.as_u64().unwrap() as u8)
                            .collect(),
                    })
                    .collect(),
            };
            assert_eq!(bundle::encode(&expected), bytes);
            assert_eq!(bundle::decode(&bytes).unwrap(), expected);
        } else {
            let expected = SetupBundle {
                setup_id: value["setupId"].as_u64().unwrap() as u32,
                entries: entries
                    .iter()
                    .map(|entry| SetupEntry {
                        id: entry["id"].as_u64().unwrap() as u32,
                        data: entry["data"].as_array().map(|data| {
                            data.iter()
                                .map(|byte| byte.as_u64().unwrap() as u8)
                                .collect()
                        }),
                    })
                    .collect(),
            };
            assert_eq!(setup_bundle::encode(&expected).unwrap(), bytes);
            assert_eq!(setup_bundle::decode(&bytes).unwrap(), expected);
        }
    }
}
