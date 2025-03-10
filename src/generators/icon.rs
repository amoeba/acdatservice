use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, RgbaImage};
use worker::*;

const ICON_DIM: u32 = 32;

pub async fn generate_dat_image(object: Object, scale: u32) -> Result<Response> {
    let body = object.body().unwrap();
    let actual_body = body.bytes().await?;
    let mut png_buffer = Vec::new();

    let img: RgbaImage = ImageBuffer::from_raw(ICON_DIM, ICON_DIM, actual_body)
        .expect("Failed to create ImageBuffer");
    let dynamic_img: DynamicImage = DynamicImage::ImageRgba8(img).resize(
        ICON_DIM * scale,
        ICON_DIM * scale,
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
