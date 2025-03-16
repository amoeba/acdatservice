use std::{collections::HashMap, io::Cursor};
use libac_rs::{dat::file_types::texture::Texture, icon::Icon};
use worker::*;
use deku::DekuContainerRead;

use crate::{
    db, generators::icon::generate_icon, openapi::{Contact, Info, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server}
};

pub async fn index_get(_ctx: RouteContext<()>) -> Result<Response> {
    let mut paths = HashMap::new();
    paths.insert(
        "/icons/:short_id".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get an icon".to_string(),
                description: "Returns a PNG icon with optional scaling applied. Example https://dats.treestats.net/icons/26967?scale=2.".to_string(),
                operation_id: "icons_get".to_string(),
                parameters: vec![Parameter {
                    name: "scale".to_string(),
                    location: "query".to_string(),
                    description: "Optional integer value to scale the image by".to_string(),
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

pub async fn get_buf_for_file(ctx: &RouteContext<()>, file: &db::File)-> std::result::Result<Vec<u8>, worker::Error> {
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
        Some(obj) => {
            Ok(obj.body().unwrap().bytes().await?)
        }
        None => todo!(),
    }
}

pub async fn get_file(ctx: &RouteContext<()>, file_id: u32) -> Result<Option<db::File>> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id_short = ?1 LIMIT 1");
    let query = statement.bind(&[file_id.into()])?;

    Ok(query.first::<crate::db::File>(None).await?)
}

pub async fn icons_get(url: Url, ctx: RouteContext<()>) -> Result<Response>{
    let query_params = url.query_pairs();

    // Scale
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

    // Icon ID
    let param_id = match ctx.param("id") {
        Some(val) => val,
        None => return Response::error("Must specify icon ID.", 400),
    };

    let param_id_num = match param_id.parse::<u32>() {
        Ok(val) => val,
        Err(_) => return Response::error("Failed to parse icon as ObjectID", 400),
    };

    // Underlay
    let param_underlay = match query_params
        .clone()
        .find(|(key, _)| key == "underlay") {
            Some((_, value)) => {
                match value.parse::<u32>() {
                    Ok(value) => Ok(Some(value)),
                    Err(_) => Err("Failed to parse underlay parameter as u32".to_string())
                }
            },
            None => Ok(None)
    }?;

    let maybe_underlay = match param_underlay {
        Some(val) => {
            let underlay_file = match get_file(&ctx, val).await? {
                Some(val) => val,
                None=> return Response::error("Failed to get file", 400)
            };

            // Create icon
            let underlay_object = get_buf_for_file(&ctx, &underlay_file).await?;

            let mut reader = Cursor::new(underlay_object);
            let underlay_texture = match Texture::from_reader((&mut reader, 0)) {
                Ok(val) => val,
                Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
            };

            Some(underlay_texture.1)

        },
        None => None,
    };

    // Look up Icon by ID against D1 Database
    let base_file = match get_file(&ctx, param_id_num).await? {
        Some(val) => val,
        None=> return Response::error("Failed to get file", 400)
    };

    // Create icon
    let base_object = get_buf_for_file(&ctx, &base_file).await?;

    // Debugging
    // let mut response = Response::from_body(worker::ResponseBody::Body(base_object))?;
    // response.headers_mut().set("Content-Type", "application/octet-stream")?;
    // Ok(response)

    let mut reader = Cursor::new(base_object);
    let base_texture = match Texture::from_reader((&mut reader, 0)) {
        Ok(val) => val,
        Err(e) => return Response::error(format!("Failed to instantiate : {}", e), 400),
    };

    let icon: Icon = Icon {
        width: 32,
        height: 32,
        scale: 1,
        base: base_texture.1,
        underlay: maybe_underlay,
        overlay: None,
        overlay2: None,
        effect: None,
    };

    // Generate the image or error
    generate_icon(&icon).await
}
