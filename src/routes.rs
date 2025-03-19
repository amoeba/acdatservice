use libac_rs::{dat::file_types::texture::Texture, icon::Icon};
use std::{collections::HashMap, io::Cursor};
use worker::*;

use crate::{
    db,
    generators::icon::generate_icon,
    openapi::{Contact, Info, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server},
};

pub async fn index_get(_ctx: RouteContext<()>) -> Result<Response> {
    let mut paths = HashMap::new();
    paths.insert(
        "/icons/:icon_id".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get an icon".to_string(),
                description: "Returns a PNG icon with optional scaling applied. Example https://dats.treestats.net/icons/26967?scale=2.".to_string(),
                operation_id: "icons_get".to_string(),
                parameters: vec![Parameter {
                    name: "icon_id".to_string(),
                    location: "path".to_string(),
                    description: "Icon ID as decimal or hex, absolute or relative.".to_string(),
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

pub async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    let _ = ctx;
    Response::ok("See / for OpenAPI spec.")
}

pub async fn get_buf_for_file(
    ctx: &RouteContext<()>,
    file: &db::File,
) -> std::result::Result<Vec<u8>, worker::Error> {
    let bucket = ctx.bucket("DATS_BUCKET")?;
    let builder = bucket.get("client_portal.dat");
    let data = builder
        .range(Range::OffsetWithLength {
            offset: file.offset + 8, // 8 is the first two DWORDS before the texture
            length: file.size as u64,
        })
        .execute()
        .await?;

    match data {
        Some(obj) => Ok(obj.body().unwrap().bytes().await?),
        None => todo!(),
    }
}

pub async fn get_file(ctx: &RouteContext<()>, file_id: u32) -> Result<Option<db::File>> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id_short = ?1 LIMIT 1");
    let query = statement.bind(&[file_id.into()])?;

    Ok(query.first::<crate::db::File>(None).await?)
}

pub async fn icons_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
    let query_params = url.query_pairs();

    // Icon ID
    let param_id = match ctx.param("id") {
        Some(val) => val,
        None => return Response::error("Must specify icon ID.", 400),
    };

    let param_id_num = match param_id.parse::<u32>() {
        Ok(val) => val,
        Err(_) => return Response::error("Failed to parse icon as ObjectID", 400),
    };

    // scale
    let param_scale = match query_params
        .clone()
        .find(|(key, _)| key == "scale")
        .map(|(_, value)| value.parse::<u32>())
        .unwrap_or_else(|| Ok(1))
    {
        Ok(val) => val,
        Err(err) => {
            return Response::error(format!("Bad request: {}", err), 400);
        }
    };

    // Error for unreasonable scale values
    if param_scale < 1 || param_scale > 8 {
        return Response::error("Choose a scale value between 1 and 8", 400);
    }

    // underlay
    let param_underlay = match query_params.clone().find(|(key, _)| key == "underlay") {
        Some((_, value)) => match value.parse::<u32>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err("Failed to parse underlay parameter as u32".to_string()),
        },
        None => Ok(None),
    }?;

    // overlay
    let param_overlay = match query_params.clone().find(|(key, _)| key == "overlay") {
        Some((_, value)) => match value.parse::<u32>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err("Failed to parse overlay parameter as u32".to_string()),
        },
        None => Ok(None),
    }?;

    // overlay2
    let param_overlay2 = match query_params.clone().find(|(key, _)| key == "overlay2") {
        Some((_, value)) => match value.parse::<u32>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err("Failed to parse overlay2 parameter as u32".to_string()),
        },
        None => Ok(None),
    }?;

    // ui_effect
    let param_ui_effect = match query_params.clone().find(|(key, _)| key == "ui_effect") {
        Some((_, value)) => match value.parse::<u32>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err("Failed to parse ui_effect parameter as u32".to_string()),
        },
        None => Ok(None),
    }?;

    // Get textures for any files we need
    let maybe_underlay = match param_underlay {
        Some(val) => {
            let underlay_file = match get_file(&ctx, val).await? {
                Some(val) => val,
                None => return Response::error("Failed to get file", 400),
            };

            // Create icon
            let underlay_object = get_buf_for_file(&ctx, &underlay_file).await?;

            let mut reader = Cursor::new(underlay_object);
            let underlay_texture = match Texture::read(&mut reader) {
                Ok(val) => val,
                Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
            };

            Some(underlay_texture)
        }
        None => None,
    };

    let maybe_overlay = match param_overlay {
        Some(val) => {
            let overlay_file = match get_file(&ctx, val).await? {
                Some(val) => val,
                None => return Response::error("Failed to get file", 400),
            };

            let overlay_object = get_buf_for_file(&ctx, &overlay_file).await?;

            let mut reader = Cursor::new(overlay_object);
            let overlay_texture = match Texture::read(&mut reader) {
                Ok(val) => val,
                Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
            };

            Some(overlay_texture)
        }
        None => None,
    };

    let maybe_overlay2 = match param_overlay2 {
        Some(val) => {
            let overlay2_file = match get_file(&ctx, val).await? {
                Some(val) => val,
                None => return Response::error("Failed to get file", 400),
            };

            let overlay2_object = get_buf_for_file(&ctx, &overlay2_file).await?;

            let mut reader = Cursor::new(overlay2_object);
            let overlay2_texture = match Texture::read(&mut reader) {
                Ok(val) => val,
                Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
            };

            Some(overlay2_texture)
        }
        None => None,
    };

    let maybe_ui_effect = match param_ui_effect {
        Some(val) => {
            let effect_file = match get_file(&ctx, val).await? {
                Some(val) => val,
                None => return Response::error("Failed to get file", 400),
            };

            let effect_object = get_buf_for_file(&ctx, &effect_file).await?;

            let mut reader = Cursor::new(effect_object);
            let effect_texture = match Texture::read(&mut reader) {
                Ok(val) => val,
                Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
            };

            Some(effect_texture)
        }
        None => None,
    };

    // Look up Icon by ID against D1 Database
    let base_file = match get_file(&ctx, param_id_num).await? {
        Some(val) => val,
        None => return Response::error("Failed to get file", 400),
    };

    // Create icon
    let base_object = get_buf_for_file(&ctx, &base_file).await?;

    // Debugging
    // let mut response = Response::from_body(worker::ResponseBody::Body(base_object))?;
    // response.headers_mut().set("Content-Type", "application/octet-stream")?;
    // Ok(response)

    let mut reader = Cursor::new(base_object);
    let base_texture = match Texture::read(&mut reader) {
        Ok(val) => val,
        Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
    };

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
