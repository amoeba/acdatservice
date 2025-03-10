use std::collections::HashMap;
use worker::*;

use crate::{
    generators::icon::generate_dat_image,
    openapi::{Contact, Info, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server},
};

pub async fn index_get(_ctx: RouteContext<()>) -> Result<Response> {
    let mut paths = HashMap::new();
    paths.insert(
        "/icons/:short_id".to_string(),
        PathItem {
            get: Some(Operation {
                summary: "Get an icon".to_string(),
                description: "Returns a list of users with optional filtering".to_string(),
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
pub async fn icons_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
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

    // Look up Icon by ID against D1 Database
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id_short = ?1 LIMIT 1");
    let query = statement.bind(&[param_id.into()])?;
    let result = query.first::<crate::db::File>(None).await?;

    let file = match result {
        Some(val) => val,
        None => {
            return Response::error(
                format!("Failed to find index entry for `{}`", param_id),
                404,
            )
        }
    };

    // Execute the object get operation with out byte range
    // TODO: Implement a binary reader for Textures
    let bucket = ctx.bucket("DATS_BUCKET")?;
    let object = bucket.get("client_portal.dat");
    let data = object
        .range(Range::OffsetWithLength {
            offset: file.offset + 28,
            length: 4096,
        })
        .execute()
        .await;

    // Generate the image or error
    match data {
        Ok(Some(object)) => generate_dat_image(object, param_scale).await,
        Ok(None) => return Response::error("Failed to get byte range.", 500),
        Err(err) => return Response::error(format!("Error while getting byte range: {err}"), 500),
    }
}
