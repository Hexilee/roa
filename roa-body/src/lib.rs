mod decode;
mod json;
mod mime_ext;
mod urlencoded;

use askama::Template;
use async_std::fs::File;
use async_trait::async_trait;
use futures::{AsyncBufRead as BufRead, AsyncReadExt};
use http::{HeaderValue, StatusCode};
use mime::Mime;
use mime_ext::MimeExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use roa_core::{throw, Context, Model, Status};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

#[async_trait]
pub trait PowerBody {
    /// try to get mime content type of request.
    fn request_type(&self) -> Option<Result<Mime, Status>>;

    /// try to get mime content type of response.
    fn response_type(&self) -> Option<Result<Mime, Status>>;

    /// read request body as Vec<u8>.
    async fn body_buf(&mut self) -> Result<Vec<u8>, Status>;

    /// read request body by Content-Type.
    async fn read<B: DeserializeOwned>(&mut self) -> Result<B, Status>;

    /// read request body as "application/json".
    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B, Status>;

    /// read request body as "application/x-www-form-urlencoded".
    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B, Status>;

    // read request body as "multipart/form-data"
    // async fn read_multipart(&mut self) -> Result<B, Status>;

    /// write object to response body as "application/json; charset=utf-8"
    fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result<(), Status>;

    /// write object to response body as "text/html; charset=utf-8"
    fn render<B: Template + Sync>(&mut self, data: &B) -> Result<(), Status>;

    /// write object to response body as "text/plain; charset=utf-8"
    fn write_text<S: ToString + Send>(&mut self, string: S) -> Result<(), Status>;

    /// write object to response body as "application/octet-stream"
    fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result<(), Status>;

    /// write object to response body as extension name of file
    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<(), Status>;
}

fn parse_header(value: &HeaderValue) -> Result<Mime, Status> {
    value
        .to_str()
        .map_err(|_err| {
            Status::new(
                StatusCode::BAD_REQUEST,
                "Content-Type value is not a valid utf-8 string",
                true,
            )
        })?
        .parse()
        .map_err(|err| {
            Status::new(
                StatusCode::BAD_REQUEST,
                format!("{}\nContent-Type value is invalid", err),
                true,
            )
        })
}

#[async_trait]
impl<M: Model> PowerBody for Context<M> {
    fn request_type(&self) -> Option<Result<Mime, Status>> {
        self.request
            .headers
            .get(http::header::CONTENT_TYPE)
            .map(parse_header)
    }

    fn response_type(&self) -> Option<Result<Mime, Status>> {
        self.response
            .headers
            .get(http::header::CONTENT_TYPE)
            .map(parse_header)
    }

    async fn body_buf(&mut self) -> Result<Vec<u8>, Status> {
        let mut data = Vec::new();
        self.request.read_to_end(&mut data).await?;
        Ok(data)
    }

    // return BAD_REQUEST status while parsing Content-Type fails.
    // Content-Type can only be JSON or URLENCODED, otherwise this function will return UNSUPPORTED_MEDIA_TYPE error.
    async fn read<B: DeserializeOwned>(&mut self) -> Result<B, Status> {
        match self.request_type() {
            None => self.read_json().await,
            Some(ret) => {
                let mime_type = ret?.pure_type();
                if mime_type == mime::APPLICATION_JSON {
                    self.read_json().await
                } else if mime_type == mime::APPLICATION_WWW_FORM_URLENCODED {
                    self.read_form().await
                } else {
                    throw(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Content-Type can only be JSON or URLENCODED",
                    )
                }
            }
        }
    }

    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B, Status> {
        let data = self.body_buf().await?;
        match self.request_type() {
            None | Some(Err(_)) => json::from_bytes(&data),
            Some(Ok(mime_type)) => {
                if mime_type.pure_type() != mime::APPLICATION_JSON {
                    json::from_bytes(&data)
                } else {
                    match mime_type.get_param("charset") {
                        None | Some(mime::UTF_8) => json::from_bytes(&data),
                        Some(charset) => json::from_str(&decode::decode(&data, charset.as_str())?),
                    }
                }
            }
        }
    }

    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B, Status> {
        urlencoded::from_bytes(&self.body_buf().await?)
    }

    fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result<(), Status> {
        self.response.write_bytes(json::to_bytes(data)?);
        self.response.headers.insert(
            http::header::CONTENT_TYPE,
            "application/json; charset=utf-8".parse()?,
        );
        Ok(())
    }

    fn render<B: Template + Sync>(&mut self, data: &B) -> Result<(), Status> {
        self.response.write_str(
            data.render()
                .map_err(|err| Status::new(StatusCode::INTERNAL_SERVER_ERROR, err, false))?,
        );
        self.response.headers.insert(
            http::header::CONTENT_TYPE,
            mime::TEXT_HTML_UTF_8.as_ref().parse()?,
        );
        Ok(())
    }

    fn write_text<S: ToString + Send>(&mut self, string: S) -> Result<(), Status> {
        self.response.write_str(string.to_string());
        self.response.headers.insert(
            http::header::CONTENT_TYPE,
            mime::TEXT_PLAIN_UTF_8.as_ref().parse()?,
        );
        Ok(())
    }

    fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result<(), Status> {
        self.response.write_buf(reader);
        self.response.headers.insert(
            http::header::CONTENT_TYPE,
            mime::APPLICATION_OCTET_STREAM.as_ref().parse()?,
        );
        Ok(())
    }

    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<(), Status> {
        let path = path.as_ref();
        self.response.write(File::open(path).await?);
        self.response.headers.insert(
            http::header::CONTENT_TYPE,
            mime_guess::from_path("some_file.gif")
                .first_or_octet_stream()
                .as_ref()
                .parse()?,
        );

        if let Some(filename) = path.file_name() {
            let encoded_filename =
                utf8_percent_encode(&filename.to_string_lossy(), NON_ALPHANUMERIC).to_string();
            self.response.headers.insert(
                http::header::CONTENT_DISPOSITION,
                format!(
                    "filename={}; filename*=utf-8''{}",
                    &encoded_filename, &encoded_filename
                )
                .parse()?,
            );
        }
        Ok(())
    }
}
