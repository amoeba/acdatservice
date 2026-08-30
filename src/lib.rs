use std::error::Error;
use std::io::Cursor;

use acprotocol::dat::{
    reader::{dat_file_reader::DatFileReader, worker_r2_reader::WorkerR2RangeReader},
    DatDatabaseType,
};
use byteorder::{BigEndian, ReadBytesExt};
use counting_reader::CountingRangeReader;
use routes::{files_get, files_index, icons_get, icons_index, index_get};
use worker::*;

mod counting_reader;
mod db;
mod generators;
mod lib_test;
mod openapi;
mod routes;

fn with_cors_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.set("Access-Control-Allow-Origin", "*").ok();
    headers
        .set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .ok();
    headers
        .set("Access-Control-Allow-Headers", "Content-Type")
        .ok();
    headers
        .set("Access-Control-Expose-Headers", "X-R2-Read-Count")
        .ok();
    response
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // Handle preflight OPTIONS requests
    if req.method() == Method::Options {
        let response = Response::empty()?;
        return Ok(with_cors_headers(response));
    }

    let router = Router::new();

    let url_string = req.url()?;
    let files_url = url_string.clone();
    let icons_url = url_string.clone();
    let response = router
        .get_async("/", |_, ctx| index_get(ctx))
        .get_async("/dats/:dat/files", move |_, ctx| files_index(ctx))
        .get_async("/dats/:dat/files/:file_id", move |_, ctx| {
            files_get(files_url.clone(), ctx)
        })
        .get_async("/icons", |_, ctx| icons_index(ctx))
        .get_async("/icons/:id", move |_, ctx| {
            icons_get(icons_url.clone(), ctx)
        })
        .run(req, env)
        .await?;

    // Apply CORS headers to all responses
    Ok(with_cors_headers(response))
}

/// Parse the :dat path parameter into a database type and the corresponding R2 object key.
/// Accepts short names ("portal", "cell") and full filenames ("client_portal.dat",
/// "client_cell.dat", "client_cell_1.dat", etc.).
pub fn parse_dat_param(
    text: &str,
) -> std::result::Result<(DatDatabaseType, String), Box<dyn Error>> {
    let normalized = text.to_ascii_lowercase();

    // If the parameter already looks like a filename, use it directly as the R2 key.
    if normalized.ends_with(".dat") {
        let db_type = if normalized.contains("cell") {
            DatDatabaseType::Cell
        } else {
            DatDatabaseType::Portal
        };
        return Ok((db_type, normalized));
    }

    // Otherwise treat it as a short name and map to the actual R2 object key.
    let (db_type, object_key) = if normalized == "portal" {
        (DatDatabaseType::Portal, "client_portal.dat".to_string())
    } else if normalized == "cell" {
        // The live cell DAT is named client_cell_1.dat in both R2 and the ACE repo.
        (DatDatabaseType::Cell, "client_cell_1.dat".to_string())
    } else {
        return Err(format!("Invalid dat name: {}. Expected portal or cell.", text).into());
    };

    Ok((db_type, object_key))
}

pub async fn get_buf_for_file(
    ctx: &RouteContext<()>,
    dat_object: &str,
    file: &db::File,
) -> std::result::Result<(Vec<u8>, usize), worker::Error> {
    let bucket = ctx.bucket("DATS_BUCKET")?;
    let worker_reader = WorkerR2RangeReader::new(bucket, dat_object.to_string());
    let mut counting_reader = CountingRangeReader::new(worker_reader);
    let mut reader = DatFileReader::new(file.file_size as usize, 1024_usize)
        .map_err(|e| worker::Error::RustError(format!("Failed to create reader: {}", e)))?;
    let buf = reader
        .read_file(&mut counting_reader, file.file_offset as u32)
        .await
        .map_err(|e| worker::Error::RustError(format!("Failed to read_file: {}", e)))?;

    Ok((buf, counting_reader.count))
}

pub async fn get_file_by_id(
    ctx: &RouteContext<()>,
    database_type: DatDatabaseType,
    file_id: i32,
) -> Result<Option<db::File>> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id = ?1 AND database_type = ?2 LIMIT 1");
    // We cast to f64 to apparently work around JS
    let database_type_value = database_type.as_u32() as f64;
    let query = statement.bind(&[file_id.into(), database_type_value.into()])?;

    query.first::<crate::db::File>(None).await
}

/// Parse a file ID from decimal or hex (0x-prefixed) string.
/// Unlike parse_decimal_or_hex_string, this does not apply any icon-specific offsets.
pub fn parse_file_id(text: &str) -> std::result::Result<i32, Box<dyn Error>> {
    if let Some(hex_str) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i32::from_str_radix(hex_str, 16).map_err(|e| e.into())
    } else {
        text.parse::<i32>().map_err(|e| e.into())
    }
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
            Err(err) => Err(Box::new(err)),
        }
    }
}
