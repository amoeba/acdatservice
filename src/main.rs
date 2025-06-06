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

    let server = ServerBuilder::new(api, (), log)
        .config(config_dropshot)
        .start()
        .map_err(|error| format!("failed to start server: {}", error))?;

    server.await.map_err(|e| e.to_string())
}
