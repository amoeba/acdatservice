use openapi::{
    Contact, Info, MediaType, OpenApiDocument, Operation, Parameter, PathItem, Schema, Server,
};
use serde::Deserialize;
use std::{collections::HashMap, io::Cursor};

use image::{DynamicImage, ImageBuffer, RgbaImage};
use worker::*;

mod openapi;

async fn index_get(ctx: RouteContext<()>) -> Result<Response> {
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

async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("See / for OpenAPI spec.")
}

async fn generate_dat_image(object: Object, scale: u32) -> Result<Response> {
    let body = object.body().unwrap();
    let actual_body = body.bytes().await?;
    let mut png_buffer = Vec::new();

    let img: RgbaImage =
        ImageBuffer::from_raw(32, 32, actual_body).expect("Failed to create ImageBuffer");
    let dynamic_img: DynamicImage = DynamicImage::ImageRgba8(img).resize(
        32 * scale,
        32 * scale,
        image::imageops::FilterType::Lanczos3,
    );

    dynamic_img
        .write_to(&mut Cursor::new(&mut png_buffer), image::ImageFormat::Png)
        .expect("Failed to write image to buffer");

    let mut response = Response::from_body(worker::ResponseBody::Body(png_buffer))?;
    response.headers_mut().set("Content-Type", "image/png")?;
    response
        .headers_mut()
        .set("Content-Disposition", "inline")?;

    Ok(response)
}

#[derive(Deserialize)]
struct File {
    id: u64,
    id_short: u64,
    offset: u64,
    size: u32,
    dat_type: u32,
}

async fn icons_get(url: Url, ctx: RouteContext<()>) -> Result<Response> {
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
    let result = query.first::<File>(None).await?;

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

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();

    let url_string = req.url()?;
    router
        .get_async("/", |_, ctx| index_get(ctx))
        .get_async("/icons", |_, ctx| icons_index(ctx))
        .get_async("/icons/:id", move |_, ctx| {
            icons_get(url_string.clone(), ctx)
        })
        .run(req, env)
        .await
}
