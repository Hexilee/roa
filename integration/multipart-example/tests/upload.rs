use async_std::fs::read_to_string;
use futures::stream::TryStreamExt;
use futures::{AsyncReadExt, StreamExt};
use hyper::client::Request;
use hyper::method::Method;
use multipart::client;
use roa::http::StatusCode;
use roa::preload::*;
use roa::router::Router;
use roa::{throw, App};
use roa_multipart::Multipart;
use std::error::Error as StdError;

const FILE_PATH: &str = "../../assets/author.txt";
const FILE_NAME: &str = "author.txt";
const FIELD_NAME: &str = "file";

#[test]
fn upload() -> Result<(), Box<dyn StdError>> {
    let mut app = App::new(());
    let mut router = Router::<()>::new();
    router.post("/file", |mut ctx| async move {
        let mut form = Multipart::new(&mut ctx);
        while let Some(item) = form.next().await {
            let field = item?;
            match field.content_disposition() {
                None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
                Some(disposition) => {
                    match (disposition.get_filename(), disposition.get_name()) {
                        (Some(filename), Some(name)) => {
                            assert_eq!(FIELD_NAME, name);
                            assert_eq!(FILE_NAME, filename);
                            let mut content = String::new();
                            field.into_async_read().read_to_string(&mut content).await?;
                            let expected_content = read_to_string(FILE_PATH).await?;
                            assert_eq!(&expected_content, &content);
                        }
                        _ => throw!(StatusCode::BAD_REQUEST, "invalid field"),
                    }
                }
            }
        }
        Ok(())
    });
    let (addr, server) = app.gate(router.routes("/")?).run_local()?;
    async_std::task::spawn(server);

    // client
    let url = format!("http://{}/file", addr).parse()?;
    let request = Request::new(Method::Post, url)?;
    let mut multipart = client::Multipart::from_request(request)?;
    multipart.write_stream(
        FIELD_NAME,
        &mut std::fs::File::open(FILE_PATH)?,
        Some(FILE_NAME),
        None,
    )?;
    let resp = multipart.send()?;
    assert!(resp.status.is_success());
    Ok(())
}
