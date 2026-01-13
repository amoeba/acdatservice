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
    background: Option<String>,
    underlay: Option<String>,
    overlay: Option<String>,
    ui_effect: Option<String>,
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

    // background - accepts ID or ItemType name
    let param_background = query_params.get("background").cloned();

    // underlay - accepts ID only
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

    // overlay - accepts ID only
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

    // ui_effect - accepts ID or UiEffects name
    let param_ui_effect = query_params.get("ui_effect").cloned();

    // Helper to load texture by ID
    async fn load_texture_by_id(
        ctx: &RouteContext<()>,
        texture_id: u32,
    ) -> std::result::Result<Texture, Response> {
        let texture_file = match get_file_by_id(ctx, texture_id as i32).await {
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

        let texture_object: Vec<u8> = match get_buf_for_file(ctx, &texture_file).await {
            Ok(data) => data,
            Err(_) => {
                return Err(
                    match Response::error(
                        format!("Failed to read texture file for ID {:X}", texture_id),
                        400,
                    ) {
                        Ok(resp) => resp,
                        Err(e) => return Err(Response::from_html(format!("Error: {}", e)).unwrap()),
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
        Ok(texture_file.inner)
    }

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
            Ok(texture) => Some(texture),
            Err(response) => return Ok(response),
        }
    } else {
        None
    };

    // Load underlay if specified (ID only)
    let maybe_underlay = if let Some(underlay_id) = param_underlay {
        match load_texture_by_id(&ctx, underlay_id as u32).await {
            Ok(texture) => Some(texture),
            Err(response) => return Ok(response),
        }
    } else {
        None
    };

    // Load overlay if specified (ID only)
    let maybe_overlay = if let Some(overlay_id) = param_overlay {
        match load_texture_by_id(&ctx, overlay_id as u32).await {
            Ok(texture) => Some(texture),
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
            Ok(texture) => texture,
            Err(response) => return Ok(response),
        }
    } else {
        // Default effect (transparent)
        match load_texture_by_id(&ctx, 0x060011C5).await {
            Ok(texture) => texture,
            Err(response) => return Ok(response),
        }
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
    let icon_texture = outer_file.inner;

    let icon: Icon = Icon {
        width: 32,
        height: 32,
        scale: param_scale,
        background: maybe_background,
        underlay: maybe_underlay,
        icon: icon_texture,
        overlay: maybe_overlay,
        effect: ui_effect,
    };

    // Generate the image or error
    generate_icon(&icon).await
}
