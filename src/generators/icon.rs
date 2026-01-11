use acprotocol::dat::Icon;
use acprotocol::dat::IconExportOptions;
use worker::*;

pub async fn generate_icon(icon: &Icon) -> Result<Response> {
    console_debug!("{:?}", icon);

    let options = IconExportOptions {
        convert_white_to_black: true,
    };
    let buf = icon.export_with_options(&options)?;

    let mut response = Response::from_body(worker::ResponseBody::Body(buf))?;

    response.headers_mut().set("Content-Type", "image/png")?;
    response
        .headers_mut()
        .set("Content-Disposition", "inline")?;

    Ok(response)
}
