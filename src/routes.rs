use acprotocol::dat::{
    file_types::{dat_file::DatFile, texture::Texture},
    DatFileSubtype, Icon,
};
use std::{collections::HashMap, fmt::Debug, io::Cursor};
use worker::*;

use crate::{
    generators::icon::generate_icon,
    get_buf_for_file, get_file_by_id,
    openapi::{Contact, Info, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server},
    parse_decimal_or_hex_string,
};

#[derive(Debug)]
struct DebugResponse {
    icon_id: i32,
    scale: u32,
    underlay: Option<i32>,
    overlay: Option<i32>,
    overlay2: Option<i32>,
    ui_effect: Option<i32>,
}

pub async fn index_get(_ctx: RouteContext<()>) -> Result<Response> {
    let mut paths = HashMap::new();
    paths.insert(
        "/files".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "List all file IDs".to_string(),
                description: "Returns a newline-separated list of all file IDs in the database."
                    .to_string(),
                operation_id: "files_index".to_string(),
                parameters: vec![],
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
                    name: "underlay".to_string(),
                    location: "query".to_string(),
                    description: "Optional underlay icon ID as decimal or hex, absolute or relative.".to_string(),
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
                    description: "Optional overlay icon ID as decimal or hex, absolute or relative.".to_string(),
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
                    name: "overlay2".to_string(),
                    location: "query".to_string(),
                    description: "Optional overlay2 icon ID as decimal or hex, absolute or relative.".to_string(),
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
                    description: "Optional UIEffect icon ID as decimal or hex, absolute or relative.".to_string(),
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

    Ok(response)
}

pub async fn files_index(ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files");
    let query = statement.bind(&[])?;

    let results = query.all().await?;
    let mut file_lines = Vec::new();

    for result in results.results::<crate::db::File>()? {
        let json = serde_json::to_string(&result)?;
        file_lines.push(json);
    }

    let response_text = file_lines.join("\n");
    Response::ok(response_text)
}

pub async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE file_subtype = ?1");
    // We cast to f64 to apparently work around JS
    let icon_subtype = DatFileSubtype::Icon.as_u32() as f64;
    let query = statement.bind(&[icon_subtype.into()])?;

    let results = query.all().await?;
    let mut icon_lines = Vec::new();

    for result in results.results::<crate::db::File>()? {
        let json = serde_json::to_string(&result)?;
        icon_lines.push(json);
    }

    let response_text = icon_lines.join("\n");
    Response::ok(response_text)
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
    if param_scale < 1 || param_scale > 8 {
        return Response::error("Choose a scale value between 1 and 8", 400);
    }

    // underlay
    let param_underlay = match query_params.get("underlay") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!(
                        "Failed to parse query parameter: underlay. Error: {}",
                        err.to_string()
                    ),
                    400,
                )
            }
        },
        None => None,
    };

    // overlay
    let param_overlay = match query_params.get("overlay") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!(
                        "Failed to parse query parameter: overlay. Error: {}",
                        err.to_string()
                    ),
                    400,
                )
            }
        },
        None => None,
    };

    // overlay2
    let param_overlay2 = match query_params.get("overlay2") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!(
                        "Failed to parse query parameter: overlay2. Error: {}",
                        err.to_string()
                    ),
                    400,
                )
            }
        },
        None => None,
    };

    // ui_effect
    let param_ui_effect = match query_params.get("ui_effect") {
        Some(value) => match parse_decimal_or_hex_string(value) {
            Ok(value) => Some(value),
            Err(err) => {
                return Response::error(
                    format!(
                        "Failed to parse query parameter: ui_effect. Error: {}",
                        err.to_string()
                    ),
                    400,
                )
            }
        },
        None => None,
    };

    // Get textures for any files we need
    let maybe_underlay = match param_underlay {
        Some(val) => {
            let underlay_file = match get_file_by_id(&ctx, val).await? {
                Some(val) => val,
                None => {
                    return Response::error(
                        format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                        400,
                    )
                }
            };

            let underlay_object: Vec<u8> = get_buf_for_file(&ctx, &underlay_file).await?;
            let mut buf_reader = Cursor::new(underlay_object);
            let underlay_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
            let underlay_texture = underlay_file.inner;

            Some(underlay_texture)
        }
        None => None,
    };

    let maybe_overlay = match param_overlay {
        Some(val) => {
            let overlay_file = match get_file_by_id(&ctx, val).await? {
                Some(val) => val,
                None => {
                    return Response::error(
                        format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                        400,
                    )
                }
            };

            let overlay_object: Vec<u8> = get_buf_for_file(&ctx, &overlay_file).await?;
            let mut buf_reader = Cursor::new(overlay_object);
            let overlay_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
            let overlay_texture = overlay_file.inner;

            Some(overlay_texture)
        }
        None => None,
    };

    let maybe_overlay2 = match param_overlay2 {
        Some(val) => {
            let overlay2_file = match get_file_by_id(&ctx, val).await? {
                Some(val) => val,
                None => {
                    return Response::error(
                        format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                        400,
                    )
                }
            };

            let overlay2_object: Vec<u8> = get_buf_for_file(&ctx, &overlay2_file).await?;
            let mut buf_reader = Cursor::new(overlay2_object);
            let overlay2_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
            let overlay2_texture = overlay2_file.inner;

            Some(overlay2_texture)
        }
        None => None,
    };

    let maybe_ui_effect = match param_ui_effect {
        Some(val) => {
            let effect_file = match get_file_by_id(&ctx, val).await? {
                Some(val) => val,
                None => {
                    return Response::error(
                        format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                        400,
                    )
                }
            };

            let effect_object: Vec<u8> = get_buf_for_file(&ctx, &effect_file).await?;
            let mut buf_reader = Cursor::new(effect_object);
            let effect_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
            let effect_texture = effect_file.inner;

            Some(effect_texture)
        }
        None => None,
    };

    // Look up Icon by ID against D1 Database
    let base_file = match get_file_by_id(&ctx, param_id_num).await? {
        Some(val) => val,
        None => {
            return Response::error(
                format!("Failed to get DAT file for file with ID {:?}", param_id_num),
                400,
            )
        }
    };

    // Create icon
    let base_object = get_buf_for_file(&ctx, &base_file).await?;
    let mut buf_reader = Cursor::new(base_object);
    let outer_file: DatFile<Texture> = DatFile::read(&mut buf_reader)?;
    let base_texture = outer_file.inner;

    let icon: Icon = Icon {
        width: 32,
        height: 32,
        scale: param_scale,
        base: base_texture,
        underlay: maybe_underlay,
        overlay: maybe_overlay,
        overlay2: maybe_overlay2,
        effect: maybe_ui_effect,
    };

    // Generate the image or error
    generate_icon(&icon).await
}
