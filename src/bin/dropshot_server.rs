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

    let dat_file = DatFile {
        name: String::from(name),
    };
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

fn list_files() -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
    let db_path = "./data/index.sqlite";
    let connection = sqlite::open(db_path)?;
    let mut statement =
        connection.prepare("SELECT id, type, subtype, offset FROM files LIMIT 1024;")?;

    let mut files: Vec<FileEntry> = Vec::new();

    while let sqlite::State::Row = statement.next()? {
        let id: i64 = statement.read(0)?;
        let file_type: String = statement.read(1)?;
        let file_subtype: String = statement.read(2)?;
        let offset: i64 = statement.read(3)?;

        files.push(FileEntry {
            id: id,
            file_type: file_type,
            subtype: file_subtype,
            offset: offset,
        });
    }

    Ok(files)
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
        Err(e) => {
            return Err(HttpError::for_internal_error(
                format!("Failed to execute list files query: {:?}", e).to_string(),
            ))
        }
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

    let server = ServerBuilder::new(api, (), log)
        .config(config_dropshot)
        .start()
        .map_err(|error| format!("failed to start server: {}", error))?;

    server.await.map_err(|e| e.to_string())
}
