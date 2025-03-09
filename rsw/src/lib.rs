use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use worker::*;

#[event(fetch)]
async fn fetch(_req: HttpRequest, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let bucket = env.bucket("DATS_BUCKET").expect("Bucket not found");

    let b = bucket.get("client_portal.dat");
    let rr = b
        .range(Range::OffsetWithLength {
            offset: 632715264 + 28,
            length: 4096,
        })
        .execute()
        .await;

    match rr {
        Ok(r) => match r {
            Some(object) => {
                let body = object.body().unwrap();
                let actual_body = body.bytes().await?;

                let img: RgbaImage = ImageBuffer::from_raw(32, 32, actual_body)
                    .expect("Failed to create ImageBuffer");
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
            None => Response::error("Failed, fix this", 500),
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            Response::error("rr failed, fix this", 500)
        }
    }
}
