#[cfg(test)]
mod tests {
    use crate::{db::File, parse_decimal_or_hex_string};
    use acprotocol::dat::DatFileType;

    #[test]
    fn test_parse_icon_id_string() {
        assert_eq!(parse_decimal_or_hex_string("0xFFFF").unwrap(), 100663295);
        assert_eq!(parse_decimal_or_hex_string("0xFFFFFFFF").unwrap(), -1);

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
    fn test_parse_icon_id_string_errors() {
        assert!(parse_decimal_or_hex_string("").is_err());
        assert!(parse_decimal_or_hex_string("text").is_err());
        assert!(parse_decimal_or_hex_string("12.34").is_err());
        assert!(parse_decimal_or_hex_string("0x1").is_err());
        assert!(parse_decimal_or_hex_string("0x12345").is_err());
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
