#[cfg(test)]
mod tests {
    use crate::{
        dat_block_size,
        db::File,
        parse_dat_param, parse_decimal_or_hex_string, parse_file_id,
        routes::{
            encode_multipart_mixed, parse_file_response_format, parse_setup_include,
            parse_setup_part_ids, FileResponseFormat, MultipartPart, SetupInclude,
        },
        DatDatabaseType,
    };
    use acprotocol::dat::DatFileType;

    #[test]
    fn test_dat_block_size_matches_database_type() {
        assert_eq!(dat_block_size(DatDatabaseType::Portal), 1024);
        assert_eq!(dat_block_size(DatDatabaseType::Cell), 256);
        assert_eq!(dat_block_size(DatDatabaseType::Highres), 1024);
        assert_eq!(dat_block_size(DatDatabaseType::LocalEnglish), 1024);
    }

    #[test]
    fn test_parse_dat_param_recognizes_all_dats() {
        let expected = [
            ("portal", DatDatabaseType::Portal, "client_portal.dat"),
            ("cell", DatDatabaseType::Cell, "client_cell_1.dat"),
            ("highres", DatDatabaseType::Highres, "client_highres.dat"),
            (
                "local-english",
                DatDatabaseType::LocalEnglish,
                "client_local_English.dat",
            ),
        ];

        for (input, database_type, object_key) in expected {
            assert_eq!(
                parse_dat_param(input).unwrap(),
                (database_type, object_key.to_string())
            );
        }
    }

    #[test]
    fn test_parse_dat_param_explains_valid_names() {
        let error = parse_dat_param("textures").unwrap_err().to_string();
        assert!(error.contains("portal, cell, highres, or local_english"));
    }

    #[test]
    fn test_parse_icon_id_string() {
        assert_eq!(parse_decimal_or_hex_string("0xFFFF").unwrap(), 100663295);
        assert_eq!(parse_decimal_or_hex_string("0XFFFF").unwrap(), 100663295);
        assert_eq!(parse_decimal_or_hex_string("0xFFFFFFFF").unwrap(), -1);
        assert_eq!(parse_decimal_or_hex_string("0XFFFFFFFF").unwrap(), -1);

        assert_eq!(parse_decimal_or_hex_string("26967").unwrap(), 0x6006957);
        assert_eq!(parse_decimal_or_hex_string("100690263").unwrap(), 0x6006957);
        assert_eq!(parse_decimal_or_hex_string("0x6957").unwrap(), 0x6006957);
        assert_eq!(
            parse_decimal_or_hex_string("0x06006957").unwrap(),
            0x6006957
        );

        // This is valid for this function but will get failed later down the
        // parameter validation sequence
        assert_eq!(parse_decimal_or_hex_string("-1234").unwrap(), 100662062);

        // Test this set
        // 0x6957, 0x06006957, 26967, 100690263
        assert_eq!(parse_decimal_or_hex_string("0x6957").unwrap(), 100690263);
        assert_eq!(
            parse_decimal_or_hex_string("0x06006957").unwrap(),
            100690263
        );
        assert_eq!(parse_decimal_or_hex_string("26967").unwrap(), 100690263);
        assert_eq!(parse_decimal_or_hex_string("100690263").unwrap(), 100690263);
    }

    #[test]
    fn test_parse_all_formats_resolve_to_same_value() {
        // All four formats should resolve to the same absolute ID: 0x06000F5A = 100667226
        let expected = 0x06000F5A_i32; // 100667226 decimal

        // Short hex (4 digits) - relative, base gets added
        assert_eq!(parse_decimal_or_hex_string("0x0F5A").unwrap(), expected);
        // Long hex (8 digits) - absolute, used as-is
        assert_eq!(parse_decimal_or_hex_string("0x06000F5A").unwrap(), expected);
        // Short decimal - relative, base gets added
        assert_eq!(parse_decimal_or_hex_string("3930").unwrap(), expected); // 0x0F5A = 3930
                                                                            // Long decimal - absolute, used as-is
        assert_eq!(parse_decimal_or_hex_string("100667226").unwrap(), expected);
    }

    #[test]
    fn test_parse_icon_id_string_errors_explain_accepted_inputs() {
        for input in ["", "text", "12.34", "0x1", "0x12345", "0XNOTHEX!"] {
            let error = parse_decimal_or_hex_string(input).unwrap_err().to_string();
            assert!(error.contains("Use decimal (26967)"), "{error}");
        }
    }

    #[test]
    fn test_parse_file_id_accepts_unsigned_32_bit_values() {
        assert_eq!(parse_file_id("0xa7b2ffff").unwrap(), 2813526015);
        assert_eq!(parse_file_id("0XA7B2FFFF").unwrap(), 2813526015);
        assert_eq!(parse_file_id("2813526015").unwrap(), 2813526015);
        for input in ["-1481441281", "0x100000000", "4294967296"] {
            let error = parse_file_id(input).unwrap_err().to_string();
            assert!(error.contains("Use"), "{error}");
        }
    }

    #[test]
    fn test_parse_file_response_format_requires_an_explicit_supported_format() {
        assert_eq!(
            parse_file_response_format(None).unwrap(),
            FileResponseFormat::Binary
        );
        assert_eq!(
            parse_file_response_format(Some("json")).unwrap(),
            FileResponseFormat::Json
        );

        let error = parse_file_response_format(Some("xml")).unwrap_err();
        assert_eq!(
            error,
            "Unsupported format \"xml\". Omit format for binary data or use format=json for supported file types."
        );
    }

    #[test]
    fn test_resolved_file_type_prefers_object_id_mapping() {
        let file = File {
            id: 0x0E000002,
            database_type: 0,
            file_type: DatFileType::LandBlock.as_u32() as i64,
            file_subtype: 0,
            file_offset: 0,
            file_size: 0,
        };

        assert_eq!(file.resolved_file_type(), DatFileType::CharacterGenerator);
    }

    // ── parse_setup_include ────────────────────────────────────────────

    #[test]
    fn test_parse_setup_include_none_and_gfxobjs() {
        assert_eq!(parse_setup_include(None).unwrap(), SetupInclude::None);
        assert_eq!(
            parse_setup_include(Some("gfxobjs")).unwrap(),
            SetupInclude::GfxObjs
        );
    }

    #[test]
    fn test_parse_setup_include_rejects_unknown_values() {
        for input in ["", "GFXOBJS", "textures", "gfxobj", "gfxobjs,icons", " gfxobjs "] {
            let error = parse_setup_include(Some(input)).unwrap_err();
            assert!(
                error.contains("Unsupported include"),
                "expected rejection for input {:?}, got: {}",
                input,
                error
            );
        }
    }

    // ── parse_setup_part_ids ───────────────────────────────────────────

    /// Build a Setup binary from an ID, flags, and a list of part IDs.
    fn make_setup(setup_id: u32, flags: u32, part_ids: &[u32]) -> Vec<u8> {
        let mut data = Vec::with_capacity(12 + part_ids.len() * 4);
        data.extend_from_slice(&setup_id.to_le_bytes());
        data.extend_from_slice(&flags.to_le_bytes());
        data.extend_from_slice(&(part_ids.len() as u32).to_le_bytes());
        for part_id in part_ids {
            data.extend_from_slice(&part_id.to_le_bytes());
        }
        data
    }

    #[test]
    fn test_parse_setup_part_ids_normal_setup() {
        let setup_id = 0x02000108;
        let part_a = 0x010008AB;
        let part_b = 0x010008AC;
        let data = make_setup(setup_id, 0x0000_0001, &[part_a, part_b]);

        assert_eq!(parse_setup_part_ids(setup_id, &data).unwrap(), vec![part_a, part_b]);
    }

    #[test]
    fn test_parse_setup_part_ids_truncated_header() {
        // Only 4 bytes — the header requires 12.
        let data = b"\x08\x01\x00\x02";
        let error = parse_setup_part_ids(0x02000108, data).unwrap_err();
        assert!(error.contains("too small"), "got: {error}");
    }

    #[test]
    fn test_parse_setup_part_ids_truncated_at_header_boundary() {
        // Exactly 11 bytes — still short of the 12-byte header.
        let data = [0u8; 11];
        let error = parse_setup_part_ids(0x02000108, &data).unwrap_err();
        assert!(error.contains("too small"), "got: {error}");
    }

    #[test]
    fn test_parse_setup_part_ids_impossible_part_count() {
        let setup_id = 0x02000108;
        let part_a = 0x010008AB;
        // part_count = 1000 but only one part follows.
        let mut data = make_setup(setup_id, 0, &[part_a]);
        // Overwrite the part count to claim 1000 parts.
        data[8..12].copy_from_slice(&1000u32.to_le_bytes());

        let error = parse_setup_part_ids(setup_id, &data).unwrap_err();
        assert!(error.contains("truncat"), "got: {error}");
    }

    #[test]
    fn test_parse_setup_part_ids_huge_part_count_rejected() {
        let setup_id = 0x02000108;
        let mut data = make_setup(setup_id, 0, &[]);
        // u32::MAX parts claims ~17 GB of data — must be rejected without
        // attempting to allocate.  On 32-bit targets the checked_mul catches
        // the overflow; on 64-bit the truncation check catches it instead.
        data[8..12].copy_from_slice(&u32::MAX.to_le_bytes());

        let error = parse_setup_part_ids(setup_id, &data).unwrap_err();
        assert!(
            error.contains("too many parts")
                || error.contains("truncat")
                || error.contains("invalid parts"),
            "got: {error}"
        );
    }

    #[test]
    fn test_parse_setup_part_ids_embedded_id_mismatch() {
        let data = make_setup(0x02000109, 0, &[]);
        let error = parse_setup_part_ids(0x02000108, &data).unwrap_err();
        assert!(error.contains("mismatch"), "got: {error}");
    }

    #[test]
    fn test_parse_setup_part_ids_zero_and_duplicate_ids() {
        let setup_id = 0x02000108;
        let part = 0x010008AB;
        let data = make_setup(setup_id, 0, &[0, part, 0, part]);

        // The parser returns all IDs verbatim — deduplication and
        // zero-sentinel filtering happen in the handler, not the parser.
        assert_eq!(
            parse_setup_part_ids(setup_id, &data).unwrap(),
            vec![0, part, 0, part]
        );
    }

    #[test]
    fn test_parse_setup_part_ids_zero_part_count() {
        let setup_id = 0x02000108;
        let data = make_setup(setup_id, 0xDEAD_BEEF, &[]);

        assert_eq!(parse_setup_part_ids(setup_id, &data).unwrap(), Vec::<u32>::new());
    }

    // ── encode_multipart_mixed ─────────────────────────────────────────

    #[test]
    fn test_encode_multipart_mixed_exact_framing_and_headers() {
        let parts = vec![
            MultipartPart {
                file_id: 0x02000108,
                kind: "setup",
                data: vec![0x01, 0x02, 0x03, 0x04],
            },
            MultipartPart {
                file_id: 0x010008AB,
                kind: "gfxobj",
                data: vec![0xFF, 0xFE, 0xFD],
            },
        ];

        let (boundary, body) = encode_multipart_mixed(&parts);

        // When payload data doesn't contain the boundary candidate, the
        // suffix is 0.
        assert_eq!(boundary, "acdatservice-02000108-0");

        let mut expected = Vec::new();
        // Part 1: setup
        expected.extend_from_slice(b"--acdatservice-02000108-0\r\n");
        expected.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        expected.extend_from_slice(b"Content-ID: <0x02000108>\r\n");
        expected.extend_from_slice(b"Content-Disposition: attachment; filename=\"0x02000108.setup.bin\"\r\n");
        expected.extend_from_slice(b"Content-Location: /dats/portal/files/0x02000108\r\n");
        expected.extend_from_slice(b"\r\n");
        expected.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        expected.extend_from_slice(b"\r\n");
        // Part 2: gfxobj
        expected.extend_from_slice(b"--acdatservice-02000108-0\r\n");
        expected.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        expected.extend_from_slice(b"Content-ID: <0x010008AB>\r\n");
        expected.extend_from_slice(b"Content-Disposition: attachment; filename=\"0x010008AB.gfxobj.bin\"\r\n");
        expected.extend_from_slice(b"Content-Location: /dats/portal/files/0x010008AB\r\n");
        expected.extend_from_slice(b"\r\n");
        expected.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
        expected.extend_from_slice(b"\r\n");
        // Closing delimiter
        expected.extend_from_slice(b"--acdatservice-02000108-0--\r\n");

        assert_eq!(body, expected);
    }

    #[test]
    fn test_encode_multipart_mixed_preserves_part_order() {
        let parts = vec![
            MultipartPart {
                file_id: 0x02000108,
                kind: "setup",
                data: vec![1, 2, 3],
            },
            MultipartPart {
                file_id: 0x010008AB,
                kind: "gfxobj",
                data: vec![4, 5, 6],
            },
            MultipartPart {
                file_id: 0x010008AC,
                kind: "gfxobj",
                data: vec![7, 8, 9],
            },
        ];

        let (_boundary, body) = encode_multipart_mixed(&parts);
        let body_str = String::from_utf8(body).unwrap();

        // Content-ID headers must appear in the same order as the input parts.
        let pos_1 = body_str.find("Content-ID: <0x02000108>").unwrap();
        let pos_2 = body_str.find("Content-ID: <0x010008AB>").unwrap();
        let pos_3 = body_str.find("Content-ID: <0x010008AC>").unwrap();
        assert!(pos_1 < pos_2 && pos_2 < pos_3, "parts must preserve input order");

        // Each part's payload must appear after its own Content-ID header.
        let data_1 = body_str[pos_1..].find("\x01\x02\x03").unwrap() + pos_1;
        let data_2 = body_str[pos_2..].find("\x04\x05\x06").unwrap() + pos_2;
        let data_3 = body_str[pos_3..].find("\x07\x08\x09").unwrap() + pos_3;
        assert!(data_1 < data_2 && data_2 < data_3, "data must preserve part order");
        assert!(data_1 > pos_1 && data_2 > pos_2 && data_3 > pos_3, "data must follow its header");
    }

    #[test]
    fn test_encode_multipart_mixed_arbitrary_binary_payloads() {
        // 0x00..=0xFF covers \r, \n, \0, and every other edge-case byte.
        let binary_data: Vec<u8> = (0u8..=255).collect();

        let parts = vec![
            MultipartPart {
                file_id: 0x02000108,
                kind: "setup",
                data: binary_data.clone(),
            },
            MultipartPart {
                file_id: 0x010008AB,
                kind: "gfxobj",
                data: binary_data.clone(),
            },
        ];

        let (boundary, body) = encode_multipart_mixed(&parts);

        // The boundary must not collide with any payload byte sequence.
        let boundary_bytes = boundary.as_bytes();
        for part in &parts {
            assert!(
                !part
                    .data
                    .windows(boundary.len())
                    .any(|w| w == boundary_bytes),
                "boundary {} collides with payload data",
                boundary
            );
        }

        // The body should end with the closing delimiter.
        assert!(
            body.ends_with(format!("--{boundary}--\r\n").as_bytes()),
            "body must end with the closing boundary delimiter"
        );
    }

    #[test]
    fn test_encode_multipart_mixed_avoids_boundary_collision() {
        // The first suffix candidate (0) collides with this part's data, so
        // the encoder must advance to suffix 1.
        let parts = vec![MultipartPart {
            file_id: 0x02000108,
            kind: "setup",
            data: b"acdatservice-02000108-0".to_vec(),
        }];

        let (boundary, body) = encode_multipart_mixed(&parts);
        assert_eq!(boundary, "acdatservice-02000108-1");

        // The boundary must not appear inside the payload data.
        let boundary_bytes = boundary.as_bytes();
        assert!(!parts[0]
            .data
            .windows(boundary.len())
            .any(|w| w == boundary_bytes));

        // The body should contain the closing delimiter.
        assert!(
            body.ends_with(format!("--{boundary}--\r\n").as_bytes()),
            "body must end with the closing boundary delimiter"
        );
    }

    #[test]
    fn test_encode_multipart_mixed_empty_parts() {
        let (boundary, body) = encode_multipart_mixed(&[]);

        // With no parts, root_id is 0 and the body is only the closing delimiter.
        assert_eq!(boundary, "acdatservice-00000000-0");
        assert_eq!(body, format!("--{boundary}--\r\n").as_bytes());
    }
}
