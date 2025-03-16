use libac_rs::icon::Icon;
use worker::*;

pub async fn generate_icon(icon: &Icon) -> Result<Response> {
    let buf = icon.export()?;

    let mut response = Response::from_body(worker::ResponseBody::Body(buf))?;

    response.headers_mut().set("Content-Type", "image/png")?;
    response
        .headers_mut()
        .set("Content-Disposition", "inline")?;

    Ok(response)
}
