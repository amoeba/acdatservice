use acprotocol::dat::reader::{
    dat_file_reader::DatFileReader, worker_r2_reader::WorkerR2RangeReader,
};
use counting_reader::CountingRangeReader;
use routes::{dats_index, files_get, files_index, icons_get, icons_index, index_get, setups_get};
use std::error::Error;
use worker::*;

mod counting_reader;
mod db;
mod generators;
mod lib_test;
mod openapi;
mod routes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DatDatabaseType {
    Portal,
    Cell,
    Highres,
    LocalEnglish,
}

impl DatDatabaseType {
    pub const ALL: [Self; 4] = [Self::Portal, Self::Cell, Self::Highres, Self::LocalEnglish];

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn from_u32(value: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|database_type| database_type.as_u32() == value)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Portal => "portal",
            Self::Cell => "cell",
            Self::Highres => "highres",
            Self::LocalEnglish => "local_english",
        }
    }

    pub fn object_key(self) -> &'static str {
        match self {
            Self::Portal => "client_portal.dat",
            Self::Cell => "client_cell_1.dat",
            Self::Highres => "client_highres.dat",
            Self::LocalEnglish => "client_local_English.dat",
        }
    }

    pub fn block_size(self) -> usize {
        match self {
            Self::Cell => 256,
            Self::Portal | Self::Highres | Self::LocalEnglish => 1024,
        }
    }
}

impl std::fmt::Display for DatDatabaseType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

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
        .set(
            "Access-Control-Expose-Headers",
            "X-Limit, X-Offset, X-R2-Read-Count",
        )
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
    let files_index_url = url_string.clone();
    let files_get_url = url_string.clone();
    let icons_url = url_string.clone();
    let setups_url = url_string.clone();
    let response = router
        .get_async("/", |_, ctx| index_get(ctx))
        .get_async("/dats", |_, ctx| dats_index(ctx))
        .get_async("/dats/:dat/files", move |_, ctx| {
            files_index(files_index_url.clone(), ctx)
        })
        .get_async("/dats/:dat/files/:file_id", move |_, ctx| {
            files_get(files_get_url.clone(), ctx)
        })
        .get_async("/icons", |_, ctx| icons_index(ctx))
        .get_async("/icons/:id", move |_, ctx| {
            icons_get(icons_url.clone(), ctx)
        })
        .get_async("/setups/:id", move |_, ctx| {
            setups_get(setups_url.clone(), ctx)
        })
        .run(req, env)
        .await?;

    // Apply CORS headers to all responses
    Ok(with_cors_headers(response))
}

/// Parse the :dat path parameter into a database type and the corresponding R2 object key.
/// Accepts short names and their full client DAT filenames.
pub fn parse_dat_param(
    text: &str,
) -> std::result::Result<(DatDatabaseType, String), Box<dyn Error>> {
    let normalized = text.to_ascii_lowercase();

    let database_type = match normalized.as_str() {
        "portal" | "client_portal.dat" => DatDatabaseType::Portal,
        "cell" | "client_cell.dat" | "client_cell_1.dat" => DatDatabaseType::Cell,
        "highres" | "client_highres.dat" => DatDatabaseType::Highres,
        "local_english" | "local-english" | "client_local_english.dat" => {
            DatDatabaseType::LocalEnglish
        }
        _ => {
            return Err(format!(
                "Invalid dat name: {}. Expected portal, cell, highres, or local_english.",
                text
            )
            .into())
        }
    };

    Ok((database_type, database_type.object_key().to_string()))
}

fn dat_block_size(database_type: DatDatabaseType) -> usize {
    database_type.block_size()
}

pub async fn get_buf_for_file(
    ctx: &RouteContext<()>,
    database_type: DatDatabaseType,
    file: &db::File,
) -> std::result::Result<(Vec<u8>, usize), worker::Error> {
    let bucket = ctx.bucket("DATS_BUCKET")?;
    let worker_reader = WorkerR2RangeReader::new(bucket, database_type.object_key().to_string());
    let mut counting_reader = CountingRangeReader::new(worker_reader);
    let mut reader = DatFileReader::new(file.file_size as usize, dat_block_size(database_type))
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
    file_id: u32,
) -> Result<Option<db::File>> {
    let db = ctx.d1("DATS_DB")?;
    let statement = db.prepare("SELECT * FROM files WHERE id = ?1 AND database_type = ?2 LIMIT 1");
    // We cast to f64 to apparently work around JS
    let file_id_value = file_id as f64;
    let database_type_value = database_type.as_u32() as f64;
    let query = statement.bind(&[file_id_value.into(), database_type_value.into()])?;

    query.first::<crate::db::File>(None).await
}

/// Parse a file ID from decimal or hex (0x-prefixed) string.
/// Unlike parse_decimal_or_hex_string, this does not apply any icon-specific offsets.
pub fn parse_file_id(text: &str) -> std::result::Result<u32, Box<dyn Error>> {
    if let Some(hex_str) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u32::from_str_radix(hex_str, 16).map_err(|_| {
            format!(
                "Invalid file ID {:?}. Use a decimal ID or a hexadecimal ID such as 0x1000001.",
                text
            )
            .into()
        })
    } else {
        text.parse::<u32>().map_err(|_| {
            format!(
                "Invalid file ID {:?}. Use an unsigned decimal ID or hexadecimal ID such as 0x1000001.",
                text
            )
            .into()
        })
    }
}

/// Parse an icon or texture ID as either a decimal value or a 4-/8-digit hexadecimal value.
/// Four-digit values are relative to the icon ID range; eight-digit values are absolute.
fn parse_decimal_or_hex_string(text: &str) -> std::result::Result<i32, Box<dyn Error>> {
    const EXAMPLE: &str =
        "Use decimal (26967), relative hexadecimal (0x6957), or absolute hexadecimal (0x06006957).";

    if let Some(hex_str) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let result = match hex_str.len() {
            4 => u16::from_str_radix(hex_str, 16).map(|value| value as i16 as i32 + 0x6000000),
            8 => u32::from_str_radix(hex_str, 16).map(|value| value as i32),
            _ => {
                return Err(format!(
                "Invalid hexadecimal ID {:?}. Use exactly 4 or 8 hexadecimal digits after 0x. {}",
                text, EXAMPLE
            )
                .into())
            }
        };

        return result
            .map_err(|_| format!("Invalid hexadecimal ID {:?}. {}", text, EXAMPLE).into());
    }

    let value = text.parse::<i32>().map_err(|_| -> Box<dyn Error> {
        format!("Invalid decimal ID {:?}. {}", text, EXAMPLE).into()
    })?;

    if value < 0x6000000 {
        Ok(value + 0x6000000)
    } else {
        Ok(value)
    }
}
