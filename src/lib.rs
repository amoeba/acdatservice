use std::error::Error;
use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use routes::{icons_get, icons_index, index_get};
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
    let builder = bucket.get("client_portal.dat");
    let data = builder
        .range(Range::OffsetWithLength {
            offset: file.offset + 8, // 8 is the first two DWORDS before the texture
            length: file.size as u64,
        })
        .execute()
        .await?;

    console_debug!(
        "get_buf_for_file: file.offset = {}, file.size = {}",
        file.offset,
        file.size as u64
    );
    match data {
        Some(obj) => Ok(obj.body().unwrap().bytes().await?),
        None => {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "failed in get_buf_for_file to get buffer for the file. this code is not very debuggable").into())
        }
    }
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
                if value < 0x6957 + 0x6000000 {
                    Ok(value + 0x6000000)
                } else {
                    Ok(value)
                }
            }
            Err(err) => return Err(Box::new(err)),
        }
    }
}
