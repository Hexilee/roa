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

    // TODO: complete Response::status
    pub fn status(&mut self, status_code: StatusCode) -> &mut Self {
        unimplemented!()
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
