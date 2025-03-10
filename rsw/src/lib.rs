use serde::Deserialize;
use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use worker::*;

async fn index_get(ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("TODO:index")
}

async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("TODO: icons_index")
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
