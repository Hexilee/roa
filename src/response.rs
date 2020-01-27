use crate::Body;
use http::response::Builder;
use http::StatusCode;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

pub struct Response {
    builder: Builder,
    body: Body,
}

impl Response {
    pub fn new() -> Self {
        Self {
            builder: Builder::new(),
            body: Body::new(),
        }
    }

    pub fn status(&mut self, status_code: StatusCode) -> &mut Self {
        self.builder.status(status_code);
        self
    }

    fn into_resp(self) -> Result<http::Response<hyper::Body>, http::Error> {
        let Self { mut builder, body } = self;
        builder.body(body.stream().into())
    }
}

impl Deref for Response {
    type Target = Body;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for Response {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl TryFrom<Response> for http::Response<hyper::Body> {
    type Error = http::Error;
    fn try_from(value: Response) -> Result<Self, Self::Error> {
        value.into_resp()
    }
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Body, Response};
    use async_std::fs::File;
    use futures::AsyncReadExt;

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let mut resp = Response::new();
        let mut data = String::new();
        resp.read_to_string(&mut data).await?;
        assert_eq!("", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut resp = Response::new();
        let mut data = String::new();
        resp.write(b"Hello, World".as_ref());
        resp.read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut resp = Response::new();
        resp.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(b"World".as_ref());
        let mut data = String::new();
        resp.read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut resp = Response::new();
        resp.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(File::open("assets/test_data.txt").await?)
            .write(b".".as_ref());
        let mut data = String::new();
        resp.read_to_string(&mut data).await?;
        assert_eq!("Hello, Hexilee.", data);
        Ok(())
    }

    #[async_std::test]
    async fn response_write_str() -> std::io::Result<()> {
        let mut resp = Response::new();
        let mut data = String::new();
        resp.write_str("Hello, World");
        resp.read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }
}
