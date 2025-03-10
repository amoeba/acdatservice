use routes::{icons_get, icons_index, index_get};
use worker::*;

mod db;
mod generators;
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
