use acprotocol::dat::{
    file_types::{dat_file::DatFile, texture::Texture, CharGen, SpellTable},
    DatFileSubtype, DatFileType, Icon,
};
use std::{collections::HashMap, fmt::Debug, io::Cursor};
use worker::*;

use crate::{
    bundle::BundleKind,
    generators::icon::generate_icon,
    get_buf_for_file, get_file_by_id,
    openapi::{Contact, Info, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server},
    parse_dat_param, parse_decimal_or_hex_string, parse_file_id, with_cors_headers,
    DatDatabaseType,
};

#[allow(dead_code)]
#[derive(Debug)]
struct DebugResponse {
    icon_id: i32,
    scale: u32,
    background: Option<String>,
    underlay: Option<String>,
    overlay: Option<String>,
    ui_effect: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileResponseFormat {
    Binary,
    Json,
}

pub(crate) const DAT_ASSET_CACHE_CONTROL: &str =
    "public, max-age=86400, s-maxage=604800, stale-while-revalidate=86400";
pub(crate) const DEFAULT_PAGE_SIZE: usize = 1000;
pub(crate) const MAX_PAGE_SIZE: usize = 1000;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PaginationParams {
    pub limit: usize,
    pub offset: usize,
    pub after: Option<u32>,
}

pub(crate) fn parse_pagination(
    query_params: &HashMap<String, String>,
) -> std::result::Result<PaginationParams, String> {
    let limit = query_params
        .get("limit")
        .map(|value| value.parse::<usize>())
        .unwrap_or_else(|| Ok(DEFAULT_PAGE_SIZE))
        .map_err(|err| format!("Invalid limit: {}", err))?;
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return Err(format!("limit must be between 1 and {}", MAX_PAGE_SIZE));
    }

    let offset = query_params
        .get("offset")
        .map(|value| value.parse::<usize>())
        .unwrap_or_else(|| Ok(0))
        .map_err(|err| format!("Invalid offset: {}", err))?;

    let after = query_params
        .get("after")
        .map(|value| value.parse::<u32>())
        .transpose()
        .map_err(|err| format!("Invalid after: {}", err))?;

    Ok(PaginationParams {
        limit,
        offset,
        after,
    })
}

pub(crate) fn with_dat_asset_cache_headers(mut response: Response) -> Result<Response> {
    response
        .headers_mut()
        .set("Cache-Control", DAT_ASSET_CACHE_CONTROL)?;
    Ok(response)
}

pub(crate) fn parse_file_response_format(
    value: Option<&str>,
) -> std::result::Result<FileResponseFormat, String> {
    match value {
        None => Ok(FileResponseFormat::Binary),
        Some("json") => Ok(FileResponseFormat::Json),
        Some(value) => Err(format!(
            "Unsupported format {:?}. Omit format for binary data or use format=json for supported file types.",
            value
        )),
    }
}

/// Where `/dats/:dat/files/:file_id` reads the file bytes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileSource {
    /// Pre-extracted flat object in the EXTRACTED_BUCKET (default).
    Extracted,
    /// Original DAT in DATS_BUCKET, via D1 metadata and block-chain traversal.
    Dat,
}

pub(crate) fn parse_file_source(value: Option<&str>) -> std::result::Result<FileSource, String> {
    match value {
        None | Some("extracted") | Some("r2") => Ok(FileSource::Extracted),
        Some("dat") | Some("legacy") => Ok(FileSource::Dat),
        Some(value) => Err(format!(
            "Unsupported source {:?}. Omit source for the pre-extracted R2 object, or use source=dat for block-chain traversal of the original DAT.",
            value
        )),
    }
}

/// HTTP status and message for a file that is absent from the extracted bucket.
/// A miss is a hard error: the handler deliberately does not fall back to the
/// original DAT. Callers opt into that path explicitly with `source=dat`.
pub(crate) fn extracted_file_missing_error(file_id: u32) -> (u16, String) {
    (
        404,
        format!(
            "File not found with ID {} (0x{:08X}) in the extracted bucket. This endpoint does not fall back to the original DAT; retry with source=dat to read it from the DAT instead.",
            file_id, file_id
        ),
    )
}

/// Resolve an extracted-bucket lookup into file bytes, or an HTTP error when the
/// object is missing.
pub(crate) fn resolve_extracted_file(
    file_id: u32,
    data: Option<Vec<u8>>,
) -> std::result::Result<Vec<u8>, (u16, String)> {
    match data {
        Some(data) => Ok(data),
        None => Err(extracted_file_missing_error(file_id)),
    }
}

/// Parse a landblock identifier from a path segment.
///
/// Accepts a 16-bit landblock id (`0xB34A`, `45898`) or any full 32-bit DAT
/// file id (`0xB34AFFFF`, `0x1E3033D`); for the latter the landblock is the
/// high word. This lets both `/landblocks/0xB34A` and `/envcells/0x1E3033D`
/// resolve to the right block.
pub(crate) fn parse_bundle_block(text: &str) -> std::result::Result<u16, String> {
    let value = parse_file_id(text).map_err(|error| error.to_string())?;
    let block = if value > 0xFFFF { value >> 16 } else { value };
    Ok(block as u16)
}

/// R2 object key for a bundle: `cell/{landblocks,envcells}/0x{block:04X}.bin`.
pub(crate) fn bundle_object_key(kind: BundleKind, block: u16) -> String {
    let collection = match kind {
        BundleKind::Landblock => "landblocks",
        BundleKind::Envcell => "envcells",
    };
    format!("cell/{collection}/0x{block:04X}.bin")
}

/// R2 object key for a setup pack: `portal/setups/0x{setup_id:08X}.bin` (uppercase 8-hex mirrors the other extracted keys).
pub(crate) fn setup_bundle_object_key(setup_id: u32) -> String {
    format!("portal/setups/0x{setup_id:08X}.bin")
}

#[derive(serde::Deserialize)]
struct DatMetadataRow {
    database_type: i64,
    object_key: String,
    size_bytes: i64,
    sha256: String,
    file_count: i64,
}

#[derive(serde::Serialize)]
struct DatInfo {
    name: String,
    object_key: String,
    file_count: i64,
    size_bytes: i64,
    sha256: String,
}

#[derive(serde::Serialize)]
struct DatsResponse {
    portal: DatInfo,
    cell: DatInfo,
    highres: DatInfo,
    local_english: DatInfo,
}

/// A required `string` parameter at the given path location.
fn string_path_parameter(name: &str, description: &str) -> Parameter {
    Parameter {
        name: name.to_string(),
        location: "path".to_string(),
        description: description.to_string(),
        required: true,
        schema: Schema::ObjectSchema {
            schema_type: "string".to_string(),
            default: None,
            minimum: None,
            maximum: None,
            format: None,
            min_length: None,
            max_length: None,
            read_only: None,
            description: None,
            properties: None,
            required: vec![],
        },
    }
}

pub async fn index_get(_ctx: RouteContext<()>) -> Result<Response> {
    let mut paths = HashMap::new();
    paths.insert(
        "/dats".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List available DATs".to_string(),
                description: "Returns a JSON object describing the available DATs (portal, cell, highres, and local_english) with their R2 object keys, file counts, size in bytes, and sha256 hashes.".to_string(),
                operation_id: "dats_index".to_string(),
                parameters: vec![],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/files".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List file metadata for a DAT".to_string(),
                description: "Returns JSON Lines file metadata for the requested DAT. Use 'portal', 'cell', 'highres', or 'local_english' for the dat parameter. Results are ordered by file ID and paginated; pages default to 1000 records and are capped at 1000. For the next cursor, use the X-Next-After response header as the after value; offset is supported for compatibility.".to_string(),
                operation_id: "files_index".to_string(),
                parameters: vec![
                    Parameter {
                        name: "dat".to_string(),
                        location: "path".to_string(),
                        description: "DAT name. Use 'portal', 'cell', 'highres', or 'local_english'.".to_string(),
                        required: true,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "limit".to_string(),
                        location: "query".to_string(),
                        description: "Maximum number of files to return. Defaults to 1000, max 1000.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: Some(serde_json::json!(1000)),
                            minimum: Some(1),
                            maximum: Some(1000),
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "offset".to_string(),
                        location: "query".to_string(),
                        description: "Number of files to skip. Defaults to 0.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: Some(serde_json::json!(0)),
                            minimum: Some(0),
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "after".to_string(),
                        location: "query".to_string(),
                        description: "Return files with IDs greater than this value. More efficient than offset for sequential pagination.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: None,
                            minimum: Some(0),
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/files/{file_id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get a file by ID from a DAT".to_string(),
                description: "Returns the raw binary content of a DAT file by its ID. By default the file is served from the pre-extracted R2 bucket as a single flat object. Pass ?source=dat to read the file from the original DAT via D1 metadata and block-chain traversal instead; use this for files that have not been extracted yet or to compare against the extracted copy. The file_id can be specified as a decimal number (e.g., 16777217) or as a hex string with a 0x or 0X prefix (e.g., 0x1000001). Add ?format=json to request a JSON representation for file types that support it.".to_string(),
                operation_id: "files_get".to_string(),
                parameters: vec![
                    Parameter {
                        name: "dat".to_string(),
                        location: "path".to_string(),
                        description: "DAT name. Use 'portal', 'cell', 'highres', or 'local_english'.".to_string(),
                        required: true,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "file_id".to_string(),
                        location: "path".to_string(),
                        description: "File ID as decimal or hex (0x- or 0X-prefixed).".to_string(),
                        required: true,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "format".to_string(),
                        location: "query".to_string(),
                        description: "Optional response format. Use json to request a JSON representation for file types that support export.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "source".to_string(),
                        location: "query".to_string(),
                        description: "Optional storage source. Omit or use 'extracted' to read the pre-extracted flat R2 object; use 'dat' to read the original DAT via D1 metadata and block-chain traversal (the legacy behavior, which also returns the X-R2-Read-Count response header).".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: Some(serde_json::json!("extracted")),
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/batch".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Batch-read many files from a DAT".to_string(),
                description: "Returns a multipart/mixed response with one part per requested file ID that exists in the DAT. IDs are comma-separated decimals or 0x-prefixed hex values; duplicates are removed and order is preserved. Missing IDs are omitted (absence means not found) and never fail the batch. At most 512 ids and 8 MiB of combined payload are accepted, otherwise the request is rejected with 413.".to_string(),
                operation_id: "batch_get".to_string(),
                parameters: vec![
                    string_path_parameter("dat", "DAT name. Use 'portal', 'cell', 'highres', or 'local_english'."),
                    Parameter {
                        name: "ids".to_string(),
                        location: "query".to_string(),
                        description: "Comma-separated file IDs, each a decimal value or a 0x/0X-prefixed hexadecimal value. Duplicates are removed and first-occurrence order is preserved.".to_string(),
                        required: true,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                ],
            }),
        },
    );
    paths.insert(
        "/setups/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get a portal DAT setup".to_string(),
                description: "Returns the raw binary content of a Setup (0x02) resource from client_portal.dat. The id can be specified as a decimal number or as a hex string with a 0x or 0X prefix. Add ?include=gfxobjs to receive a multipart/mixed response whose first part is the setup and whose remaining parts are the distinct GraphicsObject (0x01) resources referenced by the setup.".to_string(),
                operation_id: "setups_get".to_string(),
                parameters: vec![
                    Parameter {
                        name: "id".to_string(),
                        location: "path".to_string(),
                        description: "Setup ID as decimal or hex (0x- or 0X-prefixed).".to_string(),
                        required: true,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "include".to_string(),
                        location: "query".to_string(),
                        description: "Optional related resources to include. Use gfxobjs to receive multipart/mixed containing the setup and its GraphicsObject parts.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "string".to_string(),
                            default: None,
                            minimum: None,
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/landblocks/{block}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get a landblock metadata bundle".to_string(),
                description: "Returns the ACLB bundle for one landblock's terrain (…FFFF) and LandblockInfo (…FFFE) from the cell DAT. The block can be a 16-bit landblock id (0xB34A) or any full file id in the block (0xB34AFFFF); the high word is used. Bytes are served from the pre-extracted R2 bucket as cell/landblocks/0x{block:04X}.bin. Only the cell DAT has bundles.".to_string(),
                operation_id: "landblocks_get".to_string(),
                parameters: vec![
                    string_path_parameter("dat", "DAT name. Only 'cell' has bundles."),
                    string_path_parameter(
                        "block",
                        "Landblock id (0xB34A) or a full file id in the block (0xB34AFFFF).",
                    ),
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/envcells/{block}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get an EnvCell bundle".to_string(),
                description: "Returns the ACLB bundle containing every EnvCell owned by one landblock from the cell DAT. The block can be a landblock id (0xB34A) or any full cell id within it (0x1E3033D); the high word is used. Bytes are served from the pre-extracted R2 bucket as cell/envcells/0x{block:04X}.bin. Only the cell DAT has bundles.".to_string(),
                operation_id: "envcells_get".to_string(),
                parameters: vec![
                    string_path_parameter("dat", "DAT name. Only 'cell' has bundles."),
                    string_path_parameter(
                        "block",
                        "Landblock id (0xB34A) or any full file id in the block (0x1E3033D).",
                    ),
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/setups/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get a precomputed setup pack".to_string(),
                description: "Returns the ACSP bundle of a Setup's full render closure, served from the extracted R2 bucket as portal/setups/0x{id:08X}.bin. Only the portal DAT has setup packs.".to_string(),
                operation_id: "setups_bundle_get".to_string(),
                parameters: vec![
                    string_path_parameter("dat", "DAT name. Only 'portal' has setup packs."),
                    string_path_parameter("id", "Setup file id (0x02001B7F or decimal)."),
                ],
            }),
        },
    );
    paths.insert(
        "/icons".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List icon metadata".to_string(),
                description: "Returns paginated JSON Lines file metadata for icon files. Pages default to 1000 records and are capped at 1000. For the next cursor, use the X-Next-After response header as the after value; offset is supported for compatibility.".to_string(),
                operation_id: "icons_index".to_string(),
                parameters: vec![
                    Parameter {
                        name: "limit".to_string(),
                        location: "query".to_string(),
                        description: "Maximum number of icons to return. Defaults to 1000, max 1000.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: Some(serde_json::json!(1000)),
                            minimum: Some(1),
                            maximum: Some(1000),
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "offset".to_string(),
                        location: "query".to_string(),
                        description: "Number of icons to skip. Defaults to 0.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: Some(serde_json::json!(0)),
                            minimum: Some(0),
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                    Parameter {
                        name: "after".to_string(),
                        location: "query".to_string(),
                        description: "Return icons with IDs greater than this value. More efficient than offset for sequential pagination.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: None,
                            minimum: Some(0),
                            maximum: None,
                            format: None,
                            min_length: None,
                            max_length: None,
                            read_only: None,
                            description: None,
                            properties: None,
                            required: vec![],
                        },
                    },
                ],
            }),
        },
    );
    paths.insert(
        "/icons/{id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get an icon".to_string(),
                description: "Returns a PNG icon with optional scaling applied and any provided underlay, overlay, or UI effect mixed in. Example https://dats.treestats.net/icons/26967?scale=2. All Icon IDs can be passed as decimal or hex and either absolute or relative (to 0x06000000) values can be used. For example, all of these values return the same icon: 0x6957, 0x06006957, 26967, 100690263.".to_string(),
                operation_id: "icons_get".to_string(),
                parameters: vec![Parameter {
                    name: "icon_id".to_string(),
                    location: "path".to_string(),
                    description: "Icon ID as decimal or hex. Accepts absolute or relative values.".to_string(),
                    required: true,
                    schema: Schema::ObjectSchema {
                        schema_type: "string".to_string(),
                        default: None,
                        minimum: None,
                        maximum: None,
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                },
                Parameter {
                    name: "scale".to_string(),
                    location: "query".to_string(),
                    description: "Optional integer value to scale the image by.".to_string(),
                    required: false,
                    schema: Schema::ObjectSchema {
                        schema_type: "integer".to_string(),
                        default: Some(serde_json::json!(1)),
                        minimum: Some(1),
                        maximum: Some(8),
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                },
                Parameter {
                    name: "background".to_string(),
                    location: "query".to_string(),
                    description: "Optional background texture. Accepts texture ID (as decimal or hex, absolute or relative) or an ItemType name (case-insensitive). ItemTypes: melee_weapon, armor, clothing, jewelry, creature, food, money, misc, missile_weapon, container, gem, spell_components, key, caster, portal, promissory_note, mana_stone, service. Use 'random' to select a random ItemType background.".to_string(),
                    required: false,
                    schema: Schema::ObjectSchema {
                        schema_type: "string".to_string(),
                        default: None,
                        minimum: None,
                        maximum: None,
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                },
                Parameter {
                    name: "underlay".to_string(),
                    location: "query".to_string(),
                    description: "Optional underlay texture ID as decimal or hex, absolute or relative.".to_string(),
                    required: false,
                    schema: Schema::ObjectSchema {
                        schema_type: "string".to_string(),
                        default: None,
                        minimum: None,
                        maximum: None,
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                },
                Parameter {
                    name: "overlay".to_string(),
                    location: "query".to_string(),
                    description: "Optional overlay texture ID as decimal or hex, absolute or relative.".to_string(),
                    required: false,
                    schema: Schema::ObjectSchema {
                        schema_type: "string".to_string(),
                        default: None,
                        minimum: None,
                        maximum: None,
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                },
                Parameter {
                    name: "ui_effect".to_string(),
                    location: "query".to_string(),
                    description: "Optional UI effect texture. Accepts texture ID (as decimal or hex, absolute or relative) or a UiEffects name (case-insensitive). UiEffects: undef (transparent), magical, poisoned, boost_health, boost_mana, boost_stamina, fire, lightning, frost, acid, bludgeoning, slashing, piercing, nether, default (fire+magical), reversed. Use 'random' to select a random UiEffect.".to_string(),
                    required: false,
                    schema: Schema::ObjectSchema {
                        schema_type: "string".to_string(),
                        default: None,
                        minimum: None,
                        maximum: None,
                        format: None,
                        min_length: None,
                        max_length: None,
                        read_only: None,
                        description: None,
                        properties: None,
                        required: vec![],
                    },
                }],
            }),
        },
    );

    let openapi_doc = OpenApiDocument {
        openapi: "3.1.1".to_string(),
        info: Info {
            title: "ACDatService API".to_string(),
            description: "API for the ACDatService".to_string(),
            version: "0.1.0".to_string(),
            contact: Contact {
                name: "Contact Info".to_string(),
                email: "petridish@gmail.com".to_string(),
                url: "https://github.com/amoeba/acdatservice".to_string(),
            },
        },
        servers: vec![Server {
            url: "https://dats.treestats.net/".to_string(),
            description: "Main ACDatService Server".to_string(),
        }],
        paths,
    };

    let json = serde_json::to_string_pretty(&openapi_doc)?;

    let mut response = Response::from_body(worker::ResponseBody::Body(json.into()))?;
    response
        .headers_mut()
        .set("Content-Type", "application/json")?;

    Ok(with_cors_headers(response))
}

pub async fn dats_index(ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DATS_DB")?;

    // DAT metadata, including the precomputed file count.
    let meta_statement =
        db.prepare("SELECT database_type, object_key, size_bytes, sha256, file_count FROM dats");
    let meta_query = meta_statement.bind(&[])?;
    let meta_results = meta_query.all().await?;
    let mut metadata: HashMap<i64, DatMetadataRow> = HashMap::new();
    for row in meta_results.results::<DatMetadataRow>()? {
        metadata.insert(row.database_type, row);
    }

    fn build_dat_info(
        name: &str,
        default_object_key: &str,
        database_type: DatDatabaseType,
        metadata: &HashMap<i64, DatMetadataRow>,
    ) -> DatInfo {
        let db_type_value = database_type.as_u32() as i64;
        let meta = metadata.get(&db_type_value);
        DatInfo {
            name: name.to_string(),
            object_key: meta
                .map(|m| m.object_key.clone())
                .unwrap_or_else(|| default_object_key.to_string()),
            file_count: meta.map(|m| m.file_count).unwrap_or(0),
            size_bytes: meta.map(|m| m.size_bytes).unwrap_or(0),
            sha256: meta
                .map(|m| m.sha256.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        }
    }

    let response = DatsResponse {
        portal: build_dat_info(
            "portal",
            "client_portal.dat",
            DatDatabaseType::Portal,
            &metadata,
        ),
        cell: build_dat_info(
            "cell",
            "client_cell_1.dat",
            DatDatabaseType::Cell,
            &metadata,
        ),
        highres: build_dat_info(
            "highres",
            "client_highres.dat",
            DatDatabaseType::Highres,
            &metadata,
        ),
        local_english: build_dat_info(
            "local_english",
            "client_local_English.dat",
            DatDatabaseType::LocalEnglish,
            &metadata,
        ),
    };

    let json = serde_json::to_string_pretty(&response)?;
    let mut response = Response::from_body(worker::ResponseBody::Body(json.into()))?;
    response
        .headers_mut()
        .set("Content-Type", "application/json")?;
    Ok(with_cors_headers(response))
}

pub async fn files_index(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();

    let param_dat = match ctx.param("dat") {
        Some(val) => val,
        None => return Response::error("Must specify DAT name.", 400),
    };

    let (database_type, _) = match parse_dat_param(param_dat) {
        Ok(val) => val,
        Err(err) => return Response::error(err.to_string(), 400),
    };

    let pagination = match parse_pagination(&query_params) {
        Ok(value) => value,
        Err(message) => return Response::error(message, 400),
    };

    let db = ctx.d1("DATS_DB")?;
    let file_columns = "id, database_type, file_type, file_subtype, file_offset, file_size";
    let statement = if pagination.after.is_some() {
        db.prepare(format!(
            "SELECT {file_columns} FROM files WHERE database_type = ?1 AND id > ?2 ORDER BY id ASC LIMIT ?3"
        ))
    } else {
        db.prepare(format!(
            "SELECT {file_columns} FROM files WHERE database_type = ?1 ORDER BY id ASC LIMIT ?2 OFFSET ?3"
        ))
    };
    // We cast to f64 to apparently work around JS
    let database_type_value = database_type.as_u32() as f64;
    let limit_value = pagination.limit as f64;
    let offset_value = pagination.offset as f64;
    let query = if let Some(after_id) = pagination.after {
        statement.bind(&[
            database_type_value.into(),
            (after_id as f64).into(),
            limit_value.into(),
        ])?
    } else {
        statement.bind(&[
            database_type_value.into(),
            limit_value.into(),
            offset_value.into(),
        ])?
    };

    let results = query.all().await?;
    let mut file_lines = Vec::new();
    let mut next_after = None;

    for result in results.results::<crate::db::File>()? {
        next_after = Some(result.id);
        let response: crate::db::FileResponse = (&result).into();
        let json = serde_json::to_string(&response)?;
        file_lines.push(json);
    }

    let response_text = file_lines.join("\n");
    let mut response = Response::ok(response_text)?;
    response
        .headers_mut()
        .set("Content-Type", "application/x-ndjson; charset=utf-8")?;
    response
        .headers_mut()
        .set("X-Limit", &pagination.limit.to_string())?;
    response
        .headers_mut()
        .set("X-Offset", &pagination.offset.to_string())?;
    if let Some(next_after) = next_after {
        response
            .headers_mut()
            .set("X-Next-After", &next_after.to_string())?;
    }
    Ok(with_cors_headers(response))
}

pub async fn icons_index(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let pagination = match parse_pagination(&query_params) {
        Ok(value) => value,
        Err(message) => return Response::error(message, 400),
    };
    let db = ctx.d1("DATS_DB")?;
    let file_columns = "id, database_type, file_type, file_subtype, file_offset, file_size";
    let statement = if pagination.after.is_some() {
        db.prepare(format!(
            "SELECT {file_columns} FROM files WHERE database_type = ?1 AND file_subtype = ?2 AND id > ?3 ORDER BY id ASC LIMIT ?4"
        ))
    } else {
        db.prepare(format!(
            "SELECT {file_columns} FROM files WHERE database_type = ?1 AND file_subtype = ?2 ORDER BY id ASC LIMIT ?3 OFFSET ?4"
        ))
    };
    // We cast to f64 to apparently work around JS
    let database_type = DatDatabaseType::Portal.as_u32() as f64;
    let icon_subtype = DatFileSubtype::Icon.as_u32() as f64;
    let query = if let Some(after_id) = pagination.after {
        statement.bind(&[
            database_type.into(),
            icon_subtype.into(),
            (after_id as f64).into(),
            (pagination.limit as f64).into(),
        ])?
    } else {
        statement.bind(&[
            database_type.into(),
            icon_subtype.into(),
            (pagination.limit as f64).into(),
            (pagination.offset as f64).into(),
        ])?
    };

    let results = query.all().await?;
    let mut icon_lines = Vec::new();
    let mut next_after = None;

    for result in results.results::<crate::db::File>()? {
        next_after = Some(result.id);
        let response: crate::db::FileResponse = (&result).into();
        let json = serde_json::to_string(&response)?;
        icon_lines.push(json);
    }

    let response_text = icon_lines.join("\n");
    let mut response = Response::ok(response_text)?;
    response
        .headers_mut()
        .set("Content-Type", "application/x-ndjson; charset=utf-8")?;
    response
        .headers_mut()
        .set("X-Limit", &pagination.limit.to_string())?;
    response
        .headers_mut()
        .set("X-Offset", &pagination.offset.to_string())?;
    if let Some(next_after) = next_after {
        response
            .headers_mut()
            .set("X-Next-After", &next_after.to_string())?;
    }
    Ok(with_cors_headers(response))
}

pub async fn files_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let param_dat = match ctx.param("dat") {
        Some(val) => val,
        None => return Response::error("Must specify DAT name.", 400),
    };

    let (database_type, _) = match parse_dat_param(param_dat) {
        Ok(val) => val,
        Err(err) => return Response::error(err.to_string(), 400),
    };

    let param_file_id = match ctx.param("file_id") {
        Some(val) => val,
        None => return Response::error("Must specify file ID.", 400),
    };

    let file_id = match parse_file_id(param_file_id) {
        Ok(val) => val,
        Err(err) => return Response::error(err.to_string(), 400),
    };

    let response_format =
        match parse_file_response_format(query_params.get("format").map(String::as_str)) {
            Ok(value) => value,
            Err(message) => return Response::error(message, 400),
        };

    let source = match parse_file_source(query_params.get("source").map(String::as_str)) {
        Ok(value) => value,
        Err(message) => return Response::error(message, 400),
    };

    // Default: read the file directly from the pre-extracted R2 bucket, stored
    // flat as {dat_name}/0x{file_id:08X}.bin by the asheron-rs CLI's
    // `dat export --raw` workflow. This replaces D1 + block-chain traversal
    // with a single R2 read. `source=dat` opts back into the old behavior for
    // files that have not been extracted/uploaded yet.
    let (file_data, known_file_type, read_count) = match source {
        FileSource::Extracted => {
            let object_key = format!("{}/0x{:08X}.bin", database_type.name(), file_id);
            let bucket = ctx.bucket("EXTRACTED_BUCKET")?;

            let object =
                bucket.get(&object_key).execute().await.map_err(|e| {
                    worker::Error::RustError(format!("Failed to read from R2: {}", e))
                })?;

            let loaded = match object {
                Some(obj) => Some(
                    obj.body()
                        .ok_or_else(|| worker::Error::RustError("Empty object body".into()))?
                        .bytes()
                        .await
                        .map_err(|e| {
                            worker::Error::RustError(format!("Failed to read object body: {}", e))
                        })?,
                ),
                None => None,
            };
            let data = match resolve_extracted_file(file_id, loaded) {
                Ok(data) => data,
                Err((status, message)) => return Response::error(message, status),
            };

            (data, None, None)
        }
        FileSource::Dat => {
            let file = match get_file_by_id(&ctx, database_type, file_id).await? {
                Some(file) => file,
                None => {
                    return Response::error(
                        format!("File not found with ID {} (0x{:X})", file_id, file_id),
                        404,
                    )
                }
            };
            let file_type = file.resolved_file_type();
            let (data, read_count) = get_buf_for_file(&ctx, database_type, &file).await?;

            (data, Some(file_type), Some(read_count))
        }
    };

    if response_format == FileResponseFormat::Json {
        // We need the file type to know how to parse. For source=extracted this
        // is a single cheap D1 query (no block reads); for source=dat we already
        // resolved it above.
        let file_type = match known_file_type {
            Some(file_type) => file_type,
            None => match get_file_by_id(&ctx, database_type, file_id).await? {
                Some(f) => f.resolved_file_type(),
                None => {
                    return Response::error(
                        format!(
                            "File metadata not found for ID {} (0x{:X})",
                            file_id, file_id
                        ),
                        404,
                    )
                }
            },
        };

        let json = match file_type {
            DatFileType::CharGen | DatFileType::CharacterGenerator => {
                let mut reader = Cursor::new(file_data.as_slice());
                reader.set_position(4);
                let chargen = CharGen::read(&mut reader).map_err(|err| {
                    worker::Error::RustError(format!(
                        "Failed to parse file {} (0x{:X}) as {}: {}",
                        file_id, file_id, file_type, err
                    ))
                })?;
                serde_json::to_string_pretty(&chargen).map_err(|err| {
                    worker::Error::RustError(format!(
                        "Failed to serialize file {} (0x{:X}) as JSON: {}",
                        file_id, file_id, err
                    ))
                })?
            }
            DatFileType::SpellTable => {
                let mut reader = Cursor::new(file_data.as_slice());
                let spell_table = SpellTable::read(&mut reader).map_err(|err| {
                    worker::Error::RustError(format!(
                        "Failed to parse file {} (0x{:X}) as {}: {}",
                        file_id, file_id, file_type, err
                    ))
                })?;
                serde_json::to_string_pretty(&spell_table).map_err(|err| {
                    worker::Error::RustError(format!(
                        "Failed to serialize file {} (0x{:X}) as JSON: {}",
                        file_id, file_id, err
                    ))
                })?
            }
            _ => {
                return Response::error(
                    format!("JSON export is not supported for file type {}", file_type),
                    400,
                )
            }
        };

        let mut response = Response::from_body(worker::ResponseBody::Body(json.into()))?;
        response
            .headers_mut()
            .set("Content-Type", "application/json")?;
        if let Some(read_count) = read_count {
            response
                .headers_mut()
                .set("X-R2-Read-Count", &read_count.to_string())?;
        }
        return Ok(with_cors_headers(response));
    }

    let mut response = Response::from_bytes(file_data)?;
    response
        .headers_mut()
        .set("Content-Type", "application/octet-stream")?;
    if let Some(read_count) = read_count {
        response
            .headers_mut()
            .set("X-R2-Read-Count", &read_count.to_string())?;
    }

    Ok(with_cors_headers(with_dat_asset_cache_headers(response)?))
}

/// Maximum number of (deduplicated) file IDs accepted by the batch route.
pub(crate) const MAX_BATCH_IDS: usize = 512;
/// Maximum combined payload bytes (excluding multipart framing) accepted by the
/// batch route, matching the `setups_get` bundle budget.
pub(crate) const MAX_BATCH_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

/// Parse the comma-separated `ids` query value into file IDs, preserving
/// first-occurrence order and dropping duplicates. Each token is a decimal
/// value or a `0x`/`0X`-prefixed hex value, parsed with [`parse_file_id`].
pub(crate) fn parse_batch_ids(raw: &str) -> std::result::Result<Vec<u32>, String> {
    let mut ids: Vec<u32> = Vec::new();
    for token in raw.split(',') {
        let token = token.trim();
        if token.is_empty() {
            return Err(
                "Invalid ids: the comma-separated list contains an empty file ID.".to_string(),
            );
        }
        let id = parse_file_id(token).map_err(|error| error.to_string())?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Reject batch requests that ask for more than [`MAX_BATCH_IDS`] distinct IDs.
pub(crate) fn check_batch_id_count(count: usize) -> std::result::Result<(), (u16, String)> {
    if count > MAX_BATCH_IDS {
        Err((
            413,
            format!(
                "Too many ids: {} requested; the maximum is {}.",
                count, MAX_BATCH_IDS
            ),
        ))
    } else {
        Ok(())
    }
}

/// Assemble a batch `multipart/mixed` response from the requested IDs and the
/// payloads that were actually found. Missing IDs are omitted; parts follow
/// the (already deduplicated) requested order. Returns an HTTP `(status, body)`
/// error tuple of `(413, message)` when the combined payload exceeds
/// [`MAX_BATCH_PAYLOAD_BYTES`].
pub(crate) fn build_batch_multipart(
    dat_name: &str,
    requested: &[u32],
    mut available: HashMap<u32, Vec<u8>>,
) -> std::result::Result<(String, Vec<u8>), (u16, String)> {
    let mut total_bytes: usize = 0;
    let mut parts = Vec::new();
    for id in requested {
        if let Some(data) = available.remove(id) {
            total_bytes = total_bytes.saturating_add(data.len());
            parts.push(MultipartPart {
                file_id: *id,
                kind: "file",
                data,
            });
        }
    }

    if total_bytes > MAX_BATCH_PAYLOAD_BYTES {
        return Err((
            413,
            format!(
                "Batch payload is {} bytes before multipart framing; the maximum is {} bytes.",
                total_bytes, MAX_BATCH_PAYLOAD_BYTES
            ),
        ));
    }

    Ok(encode_multipart_mixed_for_dat(&parts, dat_name))
}

/// `GET /dats/:dat/batch?ids=0x…,0x…`: read many files from one DAT in a single
/// `multipart/mixed` response. IDs that are absent are omitted (absence means
/// not found); the batch never 404s because one ID is missing.
pub async fn batch_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();

    let param_dat = match ctx.param("dat") {
        Some(value) => value,
        None => return Response::error("Must specify DAT name.", 400),
    };
    let (database_type, _) = match parse_dat_param(param_dat) {
        Ok(value) => value,
        Err(error) => return Response::error(error.to_string(), 400),
    };

    let raw_ids = match query_params.get("ids") {
        Some(value) => value,
        None => return Response::error("Must specify ids.", 400),
    };
    let ids = match parse_batch_ids(raw_ids) {
        Ok(ids) => ids,
        Err(message) => return Response::error(message, 400),
    };
    if ids.is_empty() {
        return Response::error("Must specify at least one file ID.", 400);
    }
    if let Err((status, message)) = check_batch_id_count(ids.len()) {
        return Response::error(message, status);
    }

    // Read each requested ID from the same pre-extracted R2 bucket that
    // `files_get` uses by default. Missing objects are skipped rather than
    // failing the whole batch.
    let bucket = ctx.bucket("EXTRACTED_BUCKET")?;
    let dat_name = database_type.name();
    let mut available: HashMap<u32, Vec<u8>> = HashMap::new();
    for &file_id in &ids {
        let object_key = format!("{}/0x{:08X}.bin", dat_name, file_id);
        let object = bucket.get(&object_key).execute().await.map_err(|error| {
            worker::Error::RustError(format!("Failed to read from R2: {}", error))
        })?;
        let Some(object) = object else {
            continue;
        };
        let bytes = object
            .body()
            .ok_or_else(|| worker::Error::RustError("Empty object body".into()))?
            .bytes()
            .await
            .map_err(|error| {
                worker::Error::RustError(format!("Failed to read object body: {}", error))
            })?;
        available.insert(file_id, bytes);
    }

    let (boundary, body) = match build_batch_multipart(dat_name, &ids, available) {
        Ok(value) => value,
        Err((status, message)) => return Response::error(message, status),
    };

    let mut response = Response::from_bytes(body)?;
    response.headers_mut().set(
        "Content-Type",
        &format!("multipart/mixed; boundary={boundary}"),
    )?;
    Ok(with_cors_headers(with_dat_asset_cache_headers(response)?))
}

/// `GET /dats/cell/landblocks/{block}`: the ACLB landblock metadata bundle.
pub async fn landblocks_get(ctx: RouteContext<()>) -> Result<Response> {
    bundle_get(ctx, BundleKind::Landblock).await
}

/// `GET /dats/cell/envcells/{block}`: the ACLB EnvCell bundle.
pub async fn envcells_get(ctx: RouteContext<()>) -> Result<Response> {
    bundle_get(ctx, BundleKind::Envcell).await
}

/// Shared handler for the two bundle routes. Bundles live in the
/// EXTRACTED_BUCKET beside the pre-extracted per-id files, under
/// `cell/{landblocks,envcells}/0x{block:04X}.bin`.
async fn bundle_get(ctx: RouteContext<()>, kind: BundleKind) -> Result<Response> {
    let param_dat = match ctx.param("dat") {
        Some(value) => value,
        None => return Response::error("Must specify DAT name.", 400),
    };
    let (database_type, _) = match parse_dat_param(param_dat) {
        Ok(value) => value,
        Err(err) => return Response::error(err.to_string(), 400),
    };
    if database_type != DatDatabaseType::Cell {
        return Response::error("Bundles are only available for the cell DAT.", 400);
    }

    let param_block = match ctx.param("block") {
        Some(value) => value,
        None => return Response::error("Must specify a landblock id.", 400),
    };
    let block = match parse_bundle_block(param_block) {
        Ok(value) => value,
        Err(message) => return Response::error(message, 400),
    };

    let object_key = bundle_object_key(kind, block);
    let bucket = ctx.bucket("EXTRACTED_BUCKET")?;
    let object =
        bucket.get(&object_key).execute().await.map_err(|error| {
            worker::Error::RustError(format!("Failed to read from R2: {error}"))
        })?;
    let Some(object) = object else {
        return Response::error(
            format!(
                "No bundle at {object_key}. Generate it with `acdatservice generate` and upload it."
            ),
            404,
        );
    };
    let bytes = object
        .body()
        .ok_or_else(|| worker::Error::RustError("Empty object body".into()))?
        .bytes()
        .await
        .map_err(|error| {
            worker::Error::RustError(format!("Failed to read object body: {error}"))
        })?;

    let mut response = Response::from_bytes(bytes)?;
    response
        .headers_mut()
        .set("Content-Type", "application/octet-stream")?;
    Ok(with_cors_headers(with_dat_asset_cache_headers(response)?))
}

/// `GET /dats/portal/setups/{id}`: a precomputed ACSP setup pack.
pub async fn setups_bundle_get(ctx: RouteContext<()>) -> Result<Response> {
    let param_dat = match ctx.param("dat") {
        Some(value) => value,
        None => return Response::error("Must specify DAT name.", 400),
    };
    let (database_type, _) = match parse_dat_param(param_dat) {
        Ok(value) => value,
        Err(err) => return Response::error(err.to_string(), 400),
    };
    if database_type != DatDatabaseType::Portal {
        return Response::error("Setup packs are only available for the portal DAT.", 400);
    }

    let param_id = match ctx.param("id") {
        Some(value) => value,
        None => return Response::error("Must specify a setup id.", 400),
    };
    let id = match parse_file_id(param_id) {
        Ok(value) => value,
        Err(error) => return Response::error(error.to_string(), 400),
    };

    let key = setup_bundle_object_key(id);
    let bucket = ctx.bucket("EXTRACTED_BUCKET")?;
    let object =
        bucket.get(&key).execute().await.map_err(|error| {
            worker::Error::RustError(format!("Failed to read from R2: {error}"))
        })?;
    let Some(object) = object else {
        return Response::error(
            format!(
                "No setup pack at {key}. Generate it with the ac-noclip scripts/gen-setup-packs.ts tool and upload it to the extracted bucket."
            ),
            404,
        );
    };
    let bytes = object
        .body()
        .ok_or_else(|| worker::Error::RustError("Empty object body".into()))?
        .bytes()
        .await
        .map_err(|error| {
            worker::Error::RustError(format!("Failed to read object body: {error}"))
        })?;

    let mut response = Response::from_bytes(bytes)?;
    response
        .headers_mut()
        .set("Content-Type", "application/octet-stream")?;
    Ok(with_cors_headers(with_dat_asset_cache_headers(response)?))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupInclude {
    None,
    GfxObjs,
}

pub(crate) fn parse_setup_include(
    value: Option<&str>,
) -> std::result::Result<SetupInclude, String> {
    match value {
        None => Ok(SetupInclude::None),
        Some("gfxobjs") => Ok(SetupInclude::GfxObjs),
        Some(value) => Err(format!(
            "Unsupported include {:?}. Omit include for the setup binary or use include=gfxobjs.",
            value
        )),
    }
}

pub(crate) fn parse_setup_part_ids(
    setup_id: u32,
    setup_data: &[u8],
) -> std::result::Result<Vec<u32>, String> {
    const HEADER_SIZE: usize = 12;

    if setup_data.len() < HEADER_SIZE {
        return Err(format!(
            "Setup {} (0x{:08X}) is too small to contain its header.",
            setup_id, setup_id
        ));
    }

    let encoded_setup_id = u32::from_le_bytes(setup_data[0..4].try_into().unwrap());
    if encoded_setup_id != setup_id {
        return Err(format!(
            "Setup ID mismatch: requested {} (0x{:08X}) but the file contains {} (0x{:08X}).",
            setup_id, setup_id, encoded_setup_id, encoded_setup_id
        ));
    }

    let part_count = u32::from_le_bytes(setup_data[8..12].try_into().unwrap()) as usize;
    let part_bytes = part_count
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            format!(
                "Setup {} (0x{:08X}) has too many parts.",
                setup_id, setup_id
            )
        })?;
    let parts_end = HEADER_SIZE.checked_add(part_bytes).ok_or_else(|| {
        format!(
            "Setup {} (0x{:08X}) has an invalid parts section.",
            setup_id, setup_id
        )
    })?;

    if setup_data.len() < parts_end {
        return Err(format!(
            "Setup {} (0x{:08X}) declares {} parts but its data is truncated.",
            setup_id, setup_id, part_count
        ));
    }

    Ok((0..part_count)
        .map(|index| {
            let offset = HEADER_SIZE + index * std::mem::size_of::<u32>();
            u32::from_le_bytes(setup_data[offset..offset + 4].try_into().unwrap())
        })
        .collect())
}

#[derive(Debug)]
pub(crate) struct MultipartPart {
    pub file_id: u32,
    pub kind: &'static str,
    pub data: Vec<u8>,
}

fn multipart_boundary(parts: &[MultipartPart]) -> String {
    let root_id = parts.first().map(|part| part.file_id).unwrap_or_default();

    for suffix in 0usize.. {
        let candidate = format!("acdatservice-{:08X}-{}", root_id, suffix);
        if !parts.iter().any(|part| {
            part.data
                .windows(candidate.len())
                .any(|window| window == candidate.as_bytes())
        }) {
            return candidate;
        }
    }

    unreachable!("a multipart boundary suffix is always available")
}

/// Encode `parts` as a `multipart/mixed` body whose `Content-Location` headers
/// point at the portal DAT. Convenience wrapper over
/// [`encode_multipart_mixed_for_dat`] for the portal-only setup route.
pub(crate) fn encode_multipart_mixed(parts: &[MultipartPart]) -> (String, Vec<u8>) {
    encode_multipart_mixed_for_dat(parts, "portal")
}

/// Encode `parts` as a `multipart/mixed` body. Each `Content-Location` header
/// is parameterized to `/dats/{dat_name}/files/0x{id}` so the framing is
/// reusable across DATs (the setup route uses `"portal"`; the batch route
/// passes the requested DAT's canonical name).
pub(crate) fn encode_multipart_mixed_for_dat(
    parts: &[MultipartPart],
    dat_name: &str,
) -> (String, Vec<u8>) {
    let boundary = multipart_boundary(parts);
    let mut body = Vec::new();

    for part in parts {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-ID: <0x{:08X}>\r\nContent-Disposition: attachment; filename=\"0x{:08X}.{}.bin\"\r\nContent-Location: /dats/{dat_name}/files/0x{:08X}\r\n\r\n",
                part.file_id, part.file_id, part.kind, part.file_id
            )
            .as_bytes(),
        );
        body.extend_from_slice(&part.data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    (boundary, body)
}

async fn get_portal_files_by_ids(
    ctx: &RouteContext<()>,
    ids: &[u32],
) -> Result<HashMap<u32, crate::db::File>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = (2..=ids.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let statement = ctx.d1("DATS_DB")?.prepare(format!(
        "SELECT id, database_type, file_type, file_subtype, file_offset, file_size FROM files WHERE database_type = ?1 AND id IN ({placeholders})"
    ));
    let mut values = Vec::with_capacity(ids.len() + 1);
    values.push(D1Type::Integer(DatDatabaseType::Portal.as_u32() as i32));
    values.extend(ids.iter().map(|id| D1Type::Integer(*id as i32)));

    let results = statement.bind_refs(&values)?.all().await?;
    let mut files = HashMap::new();
    for file in results.results::<crate::db::File>()? {
        let id = u32::try_from(file.id).map_err(|_| {
            worker::Error::RustError(format!("D1 returned an invalid file ID {}.", file.id))
        })?;
        files.insert(id, file);
    }

    Ok(files)
}

pub async fn setups_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    const MAX_BUNDLE_FILE_BYTES: usize = 8 * 1024 * 1024;
    const GFXOBJ_ID_RANGE: std::ops::RangeInclusive<u32> = 0x01000000..=0x0100FFFF;

    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let include = match parse_setup_include(query_params.get("include").map(String::as_str)) {
        Ok(include) => include,
        Err(message) => return Response::error(message, 400),
    };

    let param_id = match ctx.param("id") {
        Some(value) => value,
        None => return Response::error("Must specify setup ID.", 400),
    };
    let setup_id = match parse_file_id(param_id) {
        Ok(value) => value,
        Err(error) => return Response::error(error.to_string(), 400),
    };

    let setup_file = match get_file_by_id(&ctx, DatDatabaseType::Portal, setup_id).await? {
        Some(file) if file.resolved_file_type() == DatFileType::Setup => file,
        _ => {
            return Response::error(
                format!("Setup not found with ID {} (0x{:08X})", setup_id, setup_id),
                404,
            )
        }
    };
    let (setup_data, mut read_count) =
        get_buf_for_file(&ctx, DatDatabaseType::Portal, &setup_file).await?;

    if include == SetupInclude::None {
        let mut response = Response::from_bytes(setup_data)?;
        response
            .headers_mut()
            .set("Content-Type", "application/octet-stream")?;
        response
            .headers_mut()
            .set("X-R2-Read-Count", &read_count.to_string())?;
        return Ok(with_cors_headers(with_dat_asset_cache_headers(response)?));
    }

    let part_ids = match parse_setup_part_ids(setup_id, &setup_data) {
        Ok(part_ids) => part_ids,
        Err(message) => return Response::error(message, 422),
    };
    let mut gfxobj_ids = Vec::new();
    for part_id in part_ids {
        if part_id == 0 {
            continue;
        }
        if !GFXOBJ_ID_RANGE.contains(&part_id) {
            return Response::error(
                format!(
                    "Setup {} (0x{:08X}) references non-GraphicsObject part {} (0x{:08X}).",
                    setup_id, setup_id, part_id, part_id
                ),
                422,
            );
        }
        if !gfxobj_ids.contains(&part_id) {
            gfxobj_ids.push(part_id);
        }
    }

    let gfxobj_files = get_portal_files_by_ids(&ctx, &gfxobj_ids).await?;
    let missing_or_invalid_ids: Vec<u32> = gfxobj_ids
        .iter()
        .copied()
        .filter(|id| {
            !matches!(
                gfxobj_files.get(id),
                Some(file) if file.resolved_file_type() == DatFileType::GraphicsObject
            )
        })
        .collect();
    if !missing_or_invalid_ids.is_empty() {
        let ids = missing_or_invalid_ids
            .iter()
            .map(|id| format!("0x{id:08X}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Response::error(
            format!(
                "Setup 0x{:08X} has unavailable GraphicsObject dependencies: {}.",
                setup_id, ids
            ),
            424,
        );
    }

    let total_file_bytes = gfxobj_ids.iter().try_fold(
        usize::try_from(setup_file.file_size).map_err(|_| {
            worker::Error::RustError(format!("Setup 0x{setup_id:08X} has an invalid file size."))
        })?,
        |total, id| {
            let file_size = usize::try_from(gfxobj_files[id].file_size).map_err(|_| {
                worker::Error::RustError(format!(
                    "GraphicsObject 0x{id:08X} has an invalid file size."
                ))
            })?;
            total.checked_add(file_size).ok_or_else(|| {
                worker::Error::RustError("Setup bundle size overflowed usize.".to_string())
            })
        },
    )?;
    if total_file_bytes > MAX_BUNDLE_FILE_BYTES {
        return Response::error(
            format!(
                "Setup bundle is {} bytes before multipart framing; the maximum is {} bytes.",
                total_file_bytes, MAX_BUNDLE_FILE_BYTES
            ),
            413,
        );
    }

    let mut multipart_parts = Vec::with_capacity(gfxobj_ids.len() + 1);
    multipart_parts.push(MultipartPart {
        file_id: setup_id,
        kind: "setup",
        data: setup_data,
    });
    for gfxobj_id in gfxobj_ids {
        let (data, gfxobj_read_count) =
            get_buf_for_file(&ctx, DatDatabaseType::Portal, &gfxobj_files[&gfxobj_id]).await?;
        read_count += gfxobj_read_count;
        multipart_parts.push(MultipartPart {
            file_id: gfxobj_id,
            kind: "gfxobj",
            data,
        });
    }

    let (boundary, bundle) = encode_multipart_mixed(&multipart_parts);
    let mut response = Response::from_bytes(bundle)?;
    response.headers_mut().set(
        "Content-Type",
        &format!("multipart/mixed; boundary={boundary}"),
    )?;
    response
        .headers_mut()
        .set("X-R2-Read-Count", &read_count.to_string())?;
    Ok(with_cors_headers(with_dat_asset_cache_headers(response)?))
}

pub async fn icons_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params: HashMap<_, _> = url.query_pairs().into_owned().collect();

    // :icon_id
    let param_id = match ctx.param("id") {
        Some(val) => val,
        None => return Response::error("Must specify icon ID.", 400),
    };

    let param_id_num = match parse_decimal_or_hex_string(param_id) {
        Ok(val) => val,
        Err(err) => return Response::error(err.to_string(), 400),
    };

    // scale
    let param_scale = match query_params
        .get("scale")
        .map(|value| value.parse::<u32>())
        .unwrap_or_else(|| Ok(1))
    {
        Ok(val) => val,
        Err(err) => {
            return Response::error(
                format!("Failed to parse query parameter: scale.{}", err),
                400,
            );
        }
    };

    // Error for unreasonable scale values
    if !(1..=8).contains(&param_scale) {
        return Response::error("Choose a scale value between 1 and 8", 400);
    }

    // background - accepts ID or ItemType name
    let param_background = query_params.get("background").cloned();

    // underlay - accepts ID only
    let param_underlay = match query_params.get("underlay") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!("Failed to parse query parameter: underlay. Error: {}", err),
                    400,
                )
            }
        },
        None => None,
    };

    // overlay - accepts ID only
    let param_overlay = match query_params.get("overlay") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!("Failed to parse query parameter: overlay. Error: {}", err),
                    400,
                )
            }
        },
        None => None,
    };

    // ui_effect - accepts ID or UiEffects name
    let param_ui_effect = query_params.get("ui_effect").cloned();

    // Helper to load texture by ID; returns the texture and the number of R2 reads performed.
    async fn load_texture_by_id(
        ctx: &RouteContext<()>,
        texture_id: u32,
    ) -> std::result::Result<(Texture, usize), Response> {
        let texture_file = match get_file_by_id(ctx, DatDatabaseType::Portal, texture_id).await {
            Ok(Some(file)) => file,
            _ => {
                return Err(
                    match Response::error(
                        format!("Failed to get DAT file for texture ID {:X}", texture_id),
                        400,
                    ) {
                        Ok(resp) => resp,
                        Err(e) => return Err(Response::from_html(format!("Error: {}", e)).unwrap()),
                    },
                )
            }
        };

        let (texture_object, read_count) =
            match get_buf_for_file(ctx, DatDatabaseType::Portal, &texture_file).await {
                Ok(data) => data,
                Err(_) => {
                    return Err(
                        match Response::error(
                            format!("Failed to read texture file for ID {:X}", texture_id),
                            400,
                        ) {
                            Ok(resp) => resp,
                            Err(e) => {
                                return Err(Response::from_html(format!("Error: {}", e)).unwrap())
                            }
                        },
                    )
                }
            };
        let mut buf_reader = Cursor::new(texture_object);
        let texture_file: DatFile<Texture> = match DatFile::read(&mut buf_reader) {
            Ok(file) => file,
            Err(_) => {
                return Err(
                    match Response::error("Failed to parse texture file".to_string(), 400) {
                        Ok(resp) => resp,
                        Err(e) => return Err(Response::from_html(format!("Error: {}", e)).unwrap()),
                    },
                )
            }
        };
        Ok((texture_file.inner, read_count))
    }

    let mut total_read_count: usize = 0;

    // Load background texture - can be ID or ItemType name
    let maybe_background = if let Some(bg_str) = param_background {
        // Try parsing as ID first, then as ItemType name
        let bg_texture_id = if let Ok(id) = parse_decimal_or_hex_string(&bg_str) {
            id as u32
        } else {
            // Parse as ItemType name
            match acprotocol::dat::icon::parse_item_type(&bg_str) {
                Ok(item_type_value) => {
                    acprotocol::dat::icon::get_background_from_item_type(item_type_value)
                }
                Err(e) => return Response::error(format!("Error parsing background: {}", e), 400),
            }
        };
        match load_texture_by_id(&ctx, bg_texture_id).await {
            Ok((texture, count)) => {
                total_read_count += count;
                Some(texture)
            }
            Err(response) => return Ok(response),
        }
    } else {
        None
    };

    // Load underlay if specified (ID only)
    let maybe_underlay = if let Some(underlay_id) = param_underlay {
        match load_texture_by_id(&ctx, underlay_id as u32).await {
            Ok((texture, count)) => {
                total_read_count += count;
                Some(texture)
            }
            Err(response) => return Ok(response),
        }
    } else {
        None
    };

    // Load overlay if specified (ID only)
    let maybe_overlay = if let Some(overlay_id) = param_overlay {
        match load_texture_by_id(&ctx, overlay_id as u32).await {
            Ok((texture, count)) => {
                total_read_count += count;
                Some(texture)
            }
            Err(response) => return Ok(response),
        }
    } else {
        None
    };

    // Load UI effect - can be ID or UiEffects name, defaults to transparent
    let ui_effect = if let Some(effect_str) = param_ui_effect {
        // Try parsing as ID first, then as UiEffects name
        let effect_texture_id = if let Ok(id) = parse_decimal_or_hex_string(&effect_str) {
            id as u32
        } else {
            // Parse as UiEffects name
            match acprotocol::dat::icon::parse_ui_effect(&effect_str) {
                Ok(ui_effect_flags) => {
                    acprotocol::dat::icon::get_ui_effect_texture_id(ui_effect_flags)
                }
                Err(e) => return Response::error(format!("Error parsing ui_effect: {}", e), 400),
            }
        };
        match load_texture_by_id(&ctx, effect_texture_id).await {
            Ok((texture, count)) => {
                total_read_count += count;
                texture
            }
            Err(response) => return Ok(response),
        }
    } else {
        // Default effect (transparent)
        match load_texture_by_id(&ctx, 0x060011C5).await {
            Ok((texture, count)) => {
                total_read_count += count;
                texture
            }
            Err(response) => return Ok(response),
        }
    };

    // Look up Icon by ID against D1 Database
    let base_file = match get_file_by_id(&ctx, DatDatabaseType::Portal, param_id_num as u32).await?
    {
        Some(val) => val,
        None => {
            return Response::error(
                format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                400,
            )
        }
    };

    // Create icon
    let (base_object, base_count) =
        get_buf_for_file(&ctx, DatDatabaseType::Portal, &base_file).await?;
    total_read_count += base_count;
    let mut buf_reader = Cursor::new(base_object);
    let outer_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
    let icon_texture = outer_file.inner;

    let icon: Icon = Icon {
        width: 32,
        height: 32,
        scale: param_scale,
        background: maybe_background,
        underlay: maybe_underlay,
        icon: icon_texture,
        overlay: maybe_overlay,
        effect: Some(ui_effect),
    };

    // Generate the image or error
    let mut response = generate_icon(&icon).await?;
    response
        .headers_mut()
        .set("X-R2-Read-Count", &total_read_count.to_string())?;
    Ok(with_cors_headers(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn pagination_defaults_to_uniform_page_size() {
        assert_eq!(
            parse_pagination(&HashMap::new()).unwrap(),
            PaginationParams {
                limit: 1000,
                offset: 0,
                after: None,
            }
        );
    }

    #[test]
    fn pagination_accepts_maximum_and_cursor() {
        assert_eq!(
            parse_pagination(&params(&[
                ("limit", "1000"),
                ("offset", "12"),
                ("after", "42")
            ]))
            .unwrap(),
            PaginationParams {
                limit: 1000,
                offset: 12,
                after: Some(42),
            }
        );
    }

    #[test]
    fn pagination_rejects_zero_and_values_over_maximum() {
        assert!(parse_pagination(&params(&[("limit", "0")])).is_err());
        assert!(parse_pagination(&params(&[("limit", "1001")])).is_err());
    }

    #[test]
    fn pagination_rejects_invalid_values() {
        assert!(parse_pagination(&params(&[("limit", "nope")])).is_err());
        assert!(parse_pagination(&params(&[("offset", "nope")])).is_err());
        assert!(parse_pagination(&params(&[("after", "nope")])).is_err());
    }
}
