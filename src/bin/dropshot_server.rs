#![cfg(feature = "dropshot")]

use std::net::{Ipv4Addr, SocketAddr};

use dropshot::{
    endpoint, ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpError,
    HttpResponseOk, Path, RequestContext, ServerBuilder,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JsonSchema)]
struct ListOfDatFile {
    dat_files: Vec<DatFile>,
}

#[derive(Serialize, JsonSchema)]
struct DatFile {
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct GetDatFileParams {
    name: String,
}

#[endpoint(
    method = GET,
    path = "/dats",
)]
async fn myapi_dats_get(
    _rqctx: RequestContext<()>,
) -> Result<HttpResponseOk<ListOfDatFile>, HttpError> {
    let dat_files: Vec<DatFile> = vec![DatFile {
        name: String::from("example.dat"),
    }];
    Ok(HttpResponseOk(ListOfDatFile { dat_files }))
}

#[endpoint(
    method = GET,
    path = "/dats/{name}",
)]
async fn myapi_dats_get_dat_file(
    _rqctx: RequestContext<()>,
    path_params: Path<GetDatFileParams>,
) -> Result<HttpResponseOk<DatFile>, HttpError> {
    let name = path_params.into_inner().name;

    let dat_file = DatFile { name };
    Ok(HttpResponseOk(dat_file))
}

#[derive(Serialize, JsonSchema)]
struct FileEntry {
    id: i64,
    file_type: String,
    subtype: String,
    offset: i64,
}

#[derive(Serialize, JsonSchema)]
struct FilesResponse {
    files: Vec<FileEntry>,
}

fn list_files_from_connection(
    connection: &sqlite::Connection,
) -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
    let mut statement = connection.prepare(
        "SELECT files.id,
                COALESCE(file_types.name, printf('Unknown(%d)', files.file_type)),
                COALESCE(file_subtypes.name, 'None'),
                files.file_offset
         FROM files
         LEFT JOIN file_types ON file_types.id = files.file_type
         LEFT JOIN file_subtypes
           ON file_subtypes.id = files.file_subtype
          AND file_subtypes.file_type_id = files.file_type
         ORDER BY files.id ASC
         LIMIT 1024",
    )?;

    let mut files = Vec::new();
    while let sqlite::State::Row = statement.next()? {
        files.push(FileEntry {
            id: statement.read(0)?,
            file_type: statement.read(1)?,
            subtype: statement.read(2)?,
            offset: statement.read(3)?,
        });
    }

    Ok(files)
}

fn list_files() -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
    let connection = sqlite::open("./data/index.sqlite")?;
    list_files_from_connection(&connection)
}

#[endpoint(
    method = GET,
    path = "/files",
)]
async fn myapi_files_index(
    _rqctx: RequestContext<()>,
) -> Result<HttpResponseOk<FilesResponse>, HttpError> {
    // VERY WIP
    // TODO: Connect once? Maybe no need since this is fast.

    match list_files() {
        Ok(files) => {
            let response = FilesResponse { files };
            Ok(HttpResponseOk(response))
        }
        Err(e) => Err(HttpError::for_internal_error(
            format!("Failed to execute list files query: {:?}", e).to_string(),
        )),
    }
}

#[derive(Serialize, JsonSchema)]
struct RangeResult {
    start: u64,
    end: u64,
}

fn parse_range_header(header: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = header.split('=').collect();

    if parts.len() != 2 || parts[0] != "bytes" {
        return None;
    }

    let range_parts: Vec<&str> = parts[1].split('-').collect();

    if range_parts.len() != 2 {
        return None;
    }

    let start = range_parts[0].parse::<u64>().ok()?;
    let end = range_parts[1].parse::<u64>().ok()?;

    Some((start, end))
}

#[endpoint(
    method = GET,
    path = "/ranges",
)]
async fn myapi_test_byte_ranges(
    rqctx: RequestContext<()>,
) -> Result<HttpResponseOk<RangeResult>, HttpError> {
    let header = rqctx.request.headers().get("range");

    match header {
        Some(value) => {
            let range = parse_range_header(value.to_str().unwrap_or(""));

            if range.is_none() {
                return Err(HttpError::for_bad_request(
                    Some("400".to_string()),
                    "Invalid range header format.".to_string(),
                ));
            }
            let final_range = range.unwrap();

            let response = RangeResult {
                start: final_range.0,
                end: final_range.1,
            };

            Ok(HttpResponseOk(response))
        }
        None => Err(HttpError::for_bad_request(
            Some("400".to_string()),
            "Range header not passed.".to_string(),
        )),
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("dropshot-server")
    .map_err(|e| e.to_string())?;

    let config_dropshot = ConfigDropshot {
        bind_address: SocketAddr::from((Ipv4Addr::LOCALHOST, 8080)),
        ..Default::default()
    };

    let mut api = ApiDescription::new();

    api.register(myapi_dats_get).unwrap();
    api.register(myapi_dats_get_dat_file).unwrap();
    api.register(myapi_files_index).unwrap();
    api.register(myapi_test_byte_ranges).unwrap();

    let server = ServerBuilder::new(api, (), log)
        .config(config_dropshot)
        .start()
        .map_err(|error| format!("failed to start server: {}", error))?;

    server.await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_files_uses_the_current_index_schema() {
        let connection = sqlite::open(":memory:").unwrap();
        connection
            .execute(
                "CREATE TABLE files (
                    id INTEGER NOT NULL,
                    database_type INTEGER NOT NULL,
                    file_type INTEGER NOT NULL,
                    file_subtype INTEGER,
                    file_offset INTEGER NOT NULL,
                    file_size INTEGER NOT NULL
                 );
                 CREATE TABLE file_types (id INTEGER NOT NULL, name TEXT NOT NULL);
                 CREATE TABLE file_subtypes (
                    id INTEGER NOT NULL,
                    file_type_id INTEGER,
                    name TEXT NOT NULL
                 );
                 INSERT INTO file_types VALUES (12, 'Texture');
                 INSERT INTO file_subtypes VALUES (0, 12, 'Icon');
                 INSERT INTO files VALUES (2, 0, 12, 0, 200, 10);
                 INSERT INTO files VALUES (1, 0, 999, NULL, 100, 10);",
            )
            .unwrap();

        let files = list_files_from_connection(&connection).unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].id, 1);
        assert_eq!(files[0].file_type, "Unknown(999)");
        assert_eq!(files[0].subtype, "None");
        assert_eq!(files[0].offset, 100);
        assert_eq!(files[1].id, 2);
        assert_eq!(files[1].file_type, "Texture");
        assert_eq!(files[1].subtype, "Icon");
        assert_eq!(files[1].offset, 200);
    }
}
