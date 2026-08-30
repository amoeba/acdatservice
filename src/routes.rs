use acprotocol::dat::{
    file_types::{dat_file::DatFile, texture::Texture, CharGen, SpellTable},
    DatFileSubtype, DatFileType, Icon,
};
use std::{collections::HashMap, fmt::Debug, io::Cursor};
use worker::*;

use crate::{
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

#[derive(serde::Deserialize)]
struct FileCountRow {
    database_type: i64,
    count: i64,
}

#[derive(serde::Deserialize)]
struct DatMetadataRow {
    database_type: i64,
    object_key: String,
    size_bytes: i64,
    sha256: String,
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
                summary: "List file IDs for a DAT".to_string(),
                description: "Returns a newline-separated list of file IDs in the requested DAT. Use 'portal' or 'cell' for the dat parameter. Results are paginated; use ?limit and ?offset to page through large DATs.".to_string(),
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
                        description: "Maximum number of files to return. Defaults to 10000, max 100000.".to_string(),
                        required: false,
                        schema: Schema::ObjectSchema {
                            schema_type: "integer".to_string(),
                            default: Some(serde_json::json!(10000)),
                            minimum: Some(1),
                            maximum: Some(100000),
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
                ],
            }),
        },
    );
    paths.insert(
        "/dats/{dat}/files/{file_id}".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get a file by ID from a DAT".to_string(),
                description: "Returns the raw binary content of a DAT file by its ID. The file_id can be specified as a decimal number (e.g., 16777217) or as a hex string with 0x prefix (e.g., 0x1000001). Add ?format=json to request a JSON representation for file types that support it.".to_string(),
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
                        description: "File ID as decimal or hex (0x-prefixed).".to_string(),
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
                ],
            }),
        },
    );
    paths.insert(
        "/icons".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List all icon IDs".to_string(),
                description: "Returns a newline-separated list of all icon IDs in the database (files with Icon subtype).".to_string(),
                operation_id: "icons_index".to_string(),
                parameters: vec![],
            }),
        },
    );
    paths.insert(
        "/icons/:icon_id".to_string(),
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

    // File counts per DAT
    let count_statement =
        db.prepare("SELECT database_type, COUNT(*) as count FROM files GROUP BY database_type");
    let count_query = count_statement.bind(&[])?;
    let count_results = count_query.all().await?;
    let mut counts: HashMap<i64, i64> = HashMap::new();
    for row in count_results.results::<FileCountRow>()? {
        counts.insert(row.database_type, row.count);
    }

    // DAT metadata (size, sha256, object key)
    let meta_statement =
        db.prepare("SELECT database_type, object_key, size_bytes, sha256 FROM dats");
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
        counts: &HashMap<i64, i64>,
        metadata: &HashMap<i64, DatMetadataRow>,
    ) -> DatInfo {
        let db_type_value = database_type.as_u32() as i64;
        let meta = metadata.get(&db_type_value);
        DatInfo {
            name: name.to_string(),
            object_key: meta
                .map(|m| m.object_key.clone())
                .unwrap_or_else(|| default_object_key.to_string()),
            file_count: *counts.get(&db_type_value).unwrap_or(&0),
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
            &counts,
            &metadata,
        ),
        cell: build_dat_info(
            "cell",
            "client_cell_1.dat",
            DatDatabaseType::Cell,
            &counts,
            &metadata,
        ),
        highres: build_dat_info(
            "highres",
            "client_highres.dat",
            DatDatabaseType::Highres,
            &counts,
            &metadata,
        ),
        local_english: build_dat_info(
            "local_english",
            "client_local_English.dat",
            DatDatabaseType::LocalEnglish,
            &counts,
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

    let limit = match query_params
        .get("limit")
        .map(|value| value.parse::<usize>())
        .unwrap_or_else(|| Ok(10000))
    {
        Ok(val) => val,
        Err(err) => return Response::error(format!("Invalid limit: {}", err), 400),
    };

    if limit == 0 || limit > 100000 {
        return Response::error("limit must be between 1 and 100000", 400);
    }

    let offset = match query_params
        .get("offset")
        .map(|value| value.parse::<usize>())
        .unwrap_or_else(|| Ok(0))
    {
        Ok(val) => val,
        Err(err) => return Response::error(format!("Invalid offset: {}", err), 400),
    };

    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE database_type = ?1 LIMIT ?2 OFFSET ?3");
    // We cast to f64 to apparently work around JS
    let database_type_value = database_type.as_u32() as f64;
    let limit_value = limit as f64;
    let offset_value = offset as f64;
    let query = statement.bind(&[
        database_type_value.into(),
        limit_value.into(),
        offset_value.into(),
    ])?;

    let results = query.all().await?;
    let mut file_lines = Vec::new();

    for result in results.results::<crate::db::File>()? {
        let response: crate::db::FileResponse = (&result).into();
        let json = serde_json::to_string(&response)?;
        file_lines.push(json);
    }

    let response_text = file_lines.join("\n");
    let mut response = Response::ok(response_text)?;
    response.headers_mut().set("X-Limit", &limit.to_string())?;
    response
        .headers_mut()
        .set("X-Offset", &offset.to_string())?;
    Ok(with_cors_headers(response))
}

pub async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DATS_DB")?;
    let statement =
        db.prepare("SELECT * FROM files WHERE database_type = ?1 AND file_subtype = ?2");
    // We cast to f64 to apparently work around JS
    let database_type = DatDatabaseType::Portal.as_u32() as f64;
    let icon_subtype = DatFileSubtype::Icon.as_u32() as f64;
    let query = statement.bind(&[database_type.into(), icon_subtype.into()])?;

    let results = query.all().await?;
    let mut icon_lines = Vec::new();

    for result in results.results::<crate::db::File>()? {
        let response: crate::db::FileResponse = (&result).into();
        let json = serde_json::to_string(&response)?;
        icon_lines.push(json);
    }

    let response_text = icon_lines.join("\n");
    let response = Response::ok(response_text)?;
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
        Err(err) => return Response::error(format!("Invalid file ID: {}", err), 400),
    };

    let file = match get_file_by_id(&ctx, database_type, file_id).await? {
        Some(val) => val,
        None => {
            return Response::error(
                format!("File not found with ID {} (0x{:X})", file_id, file_id),
                404,
            )
        }
    };

    let (file_data, read_count) = get_buf_for_file(&ctx, database_type, &file).await?;

    if query_params.get("format").map(|value| value.as_str()) == Some("json") {
        let file_type = file.resolved_file_type();
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
        response
            .headers_mut()
            .set("X-R2-Read-Count", &read_count.to_string())?;
        return Ok(with_cors_headers(response));
    }

    let mut response = Response::from_bytes(file_data)?;
    response
        .headers_mut()
        .set("Content-Type", "application/octet-stream")?;
    response
        .headers_mut()
        .set("X-R2-Read-Count", &read_count.to_string())?;

    Ok(with_cors_headers(response))
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
        let texture_file = match get_file_by_id(ctx, DatDatabaseType::Portal, texture_id as u32)
            .await
        {
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
