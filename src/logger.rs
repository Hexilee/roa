use crate::{Body, BodyCallback, Context, Model, Next, Status, StatusKind, StatusKind::*};
use bytesize::ByteSize;
use log::{error, info};
use std::time::Instant;

pub async fn logger<M: Model>(ctx: Context<M>, next: Next) -> Result<(), Status> {
    let start = Instant::now();
    let method = ctx.method().await;
    let uri = ctx.uri().await;
    info!("--> {} {}", method, uri.path());
    let path = uri.path().to_string();
    let result = next().await;
    let status = match result {
        Ok(()) => ctx.status().await,
        Err(ref status) => status.status_code,
    };
    let callback: Box<BodyCallback> = match StatusKind::infer(status) {
        Informational | Successful | Redirection | Unknown => Box::new(move |body: &Body| {
            info!(
                "<-- {} {} {}ms {}",
                method,
                path,
                start.elapsed().as_millis(),
                ByteSize(body.consumed() as u64)
            )
        }),
        ClientError | ServerError => Box::new(move |body: &Body| {
            error!(
                "<-- {} {} {}ms {}",
                method,
                path,
                start.elapsed().as_millis(),
                ByteSize(body.consumed() as u64)
            )
        }),
    };
    ctx.resp_mut().await.on_finish(callback);
    result
}
