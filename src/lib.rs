use std::error::Error;
use std::io::Cursor;

use acprotocol::dat::reader::{
    dat_file_reader::DatFileReader, worker_r2_reader::WorkerR2RangeReader,
};
use byteorder::{BigEndian, ReadBytesExt};
use routes::{files_index, icons_get, icons_index, index_get};
use worker::*;

mod db;
mod generators;
mod lib_test;
mod openapi;
mod routes;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();

    let url_string = req.url()?;
    router
        .get_async("/", |_, ctx| index_get(ctx))
        .get_async("/files", |_, ctx| files_index(ctx))
        .get_async("/icons", |_, ctx| icons_index(ctx))
        .get_async("/icons/:id", move |_, ctx| {
            icons_get(url_string.clone(), ctx)
        })
        .run(req, env)
        .await
}

pub async fn get_buf_for_file(
    ctx: &RouteContext<()>,
    file: &db::File,
) -> std::result::Result<Vec<u8>, worker::Error> {
    let bucket = ctx.bucket("DATS_BUCKET")?;
    let mut worker_reader = WorkerR2RangeReader::new(bucket, "client_portal.dat".to_string());
    let mut reader = DatFileReader::new(file.file_size as usize, 1024 as usize)
        .map_err(|e| worker::Error::RustError(format!("Failed to create reader: {}", e)))?;
    let buf = reader
        .read_file(&mut worker_reader, file.file_offset as u32)
        .await
        .map_err(|e| worker::Error::RustError(format!("Failed to read_file: {}", e)))?;

    Ok(buf)
}

pub async fn get_file_by_id(ctx: &RouteContext<()>, file_id: i32) -> Result<Option<db::File>> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id = ?1 LIMIT 1");
    let query = statement.bind(&[file_id.into()])?;

    Ok(query.first::<crate::db::File>(None).await?)
}

fn parse_decimal_or_hex_string(text: &str) -> std::result::Result<i32, Box<dyn Error>> {
    if text.starts_with("0x") {
        let text = &text.replace("0x", "");
        let bytes = byteutils::hex_to_bytes(text)?;
        let mut reader: Cursor<&Vec<u8>> = Cursor::new(&bytes);

        let result = match text.len() {
            4 => reader.read_i16::<BigEndian>()? as i32 + 0x6000000,
            8 => reader.read_i32::<BigEndian>()?,
            _ => {
                return Err(
                    "Invalid length. Should either by 4 (0x1234) or 8 (0x12345678) hex digits."
                        .into(),
                )
            }
        };

        Ok(result)
    } else {
        // Decimal path
        let parse_result = text.parse::<i32>();

        match parse_result {
            Ok(value) => {
                if value < 0x6000000 {
                    Ok(value + 0x6000000)
                } else {
                    Ok(value)
                }
            }
            Err(err) => return Err(Box::new(err)),
        }
    }
}
