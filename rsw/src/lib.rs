use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use worker::*;

async fn index_get(ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("TODO:index")
}

async fn icons_index(ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("TODO: icons_index")
}

async fn generate_dat_image(object: Object, scale: usize) -> Result<Response> {
    // TODO: Use scale

    let body = object.body().unwrap();
    let actual_body = body.bytes().await?;

    let img: RgbaImage =
        ImageBuffer::from_raw(32, 32, actual_body).expect("Failed to create ImageBuffer");
    let dynamic_img: DynamicImage = DynamicImage::ImageRgba8(img);
    let mut png_buffer = Vec::new();
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
async fn icons_get(url: Url, ctx: RouteContext<()>, bucket: Bucket) -> Result<Response> {
    let query_params = url.query_pairs();

    // Scale
    let param_scale = match query_params
        .clone()
        .find(|(key, _)| key == "scale")
        .map(|(_, value)| value.parse::<usize>())
        .unwrap_or_else(|| Ok(1))
    {
        Ok(val) => val,
        Err(err) => {
            return Response::error(format!("Bad request: {}", err), 400);
        }
    };

    // Icon ID
    let param_id = match ctx.param("id") {
        Some(val) => val,
        None => return Response::error("Must specify icon ID.", 400),
    };

    // Look up Icon by ID against D1 Database
    // TODO

    // Convert to image
    let object = bucket.get("client_portal.dat");
    let data = object
        .range(Range::OffsetWithLength {
            offset: 632715264 + 28,
            length: 4096,
        })
        .execute()
        .await;

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
    // // Do stuff
    // let db: D1Database = env.d1("DATS_DB")?;

    // let qr = db.exec("SELECT * FROM files LIMIT 10;").await?;
    // let out = qr.as_string().unwrap().as_bytes().to_vec().clone();
    // Response::from_bytes(out)
    let bucket: Bucket = env.bucket("DATS_BUCKET").expect("Bucket not found");

    let url_string = req.url()?;
    router
        .get_async("/", |_, ctx| index_get(ctx))
        .get_async("/icons", |_, ctx| icons_index(ctx))
        .get_async("/icons/:id", move |_, ctx| {
            icons_get(url_string.clone(), ctx, bucket.clone())
        })
        .run(req, env)
        .await

    // let qr = db.exec("SELECT * FROM files LIMIT 10;").await?;

    // let bucket = env.bucket("DATS_BUCKET").expect("Bucket not found");

    // let b = bucket.get("client_portal.dat");
    // let rr = b
    //     .range(Range::OffsetWithLength {
    //         offset: 632715264 + 28,
    //         length: 4096,
    //     })
    //     .execute()
    //     .await;

    // match rr {
    //     Ok(r) => match r {
    //         Some(object) => {
    //             let body = object.body().unwrap();
    //             let actual_body = body.bytes().await?;

    //             let img: RgbaImage = ImageBuffer::from_raw(32, 32, actual_body)
    //                 .expect("Failed to create ImageBuffer");
    //             let dynamic_img: DynamicImage = DynamicImage::ImageRgba8(img);
    //             let mut png_buffer = Vec::new();
    //             dynamic_img
    //                 .write_to(&mut Cursor::new(&mut png_buffer), image::ImageFormat::Png)
    //                 .expect("Failed to write image to buffer");

    //             let mut response = Response::from_body(worker::ResponseBody::Body(png_buffer))?;
    //             response.headers_mut().set("Content-Type", "image/png")?;
    //             response
    //                 .headers_mut()
    //                 .set("Content-Disposition", "inline")?;

    //             Ok(response)
    //         }
    //         None => Response::error("Failed, fix this", 500),
    //     },
    //     Err(e) => {
    //         eprintln!("Error: {}", e);
    //         Response::error("rr failed, fix this", 500)
    //     }
    // }
}
