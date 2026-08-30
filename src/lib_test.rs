#[cfg(test)]
mod tests {
    use crate::{
        dat_block_size,
        db::File,
        parse_dat_param, parse_decimal_or_hex_string, parse_file_id,
        routes::{parse_file_response_format, FileResponseFormat},
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
}
