use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
};

use image::{DynamicImage, ImageBuffer, RgbaImage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");

    // offset: 632715264 + 28,
    // length: 4090,
    let start = 632715264 + 28;
    let len = 4096;

    let mut input = File::open("../client_portal.dat")?;

    input.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0; len];
    input.read_exact(&mut buf)?;

    println!("{:?}", buf);

    let img: RgbaImage = ImageBuffer::from_raw(32, 32, buf).expect("Failed to create ImageBuffer");
    let dynamic_img: DynamicImage = DynamicImage::ImageRgba8(img);
    let mut png_buffer = Vec::new();

    dynamic_img.write_to(&mut Cursor::new(&mut png_buffer), image::ImageFormat::Png)?;

    dynamic_img.save_with_format("out.png", image::ImageFormat::Png)?;

    Ok(())
}
