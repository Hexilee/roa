use crate::Body;
use http::{Extensions, HeaderValue, StatusCode, Version};
use hyper::HeaderMap;
use std::ops::{Deref, DerefMut};

pub struct Response {
    pub status: StatusCode,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    pub extensions: Extensions,
    body: Body,
}

impl Response {
    pub fn new() -> Self {
        Self {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            extensions: Extensions::default(),
            body: Body::new(),
        }
    }

    // TODO: complete Response::status
    pub fn status(&mut self, _status_code: StatusCode) -> &mut Self {
        unimplemented!()
    }

    fn into_resp(self) -> http::Response<hyper::Body> {
        let (mut parts, _) = http::Response::new(()).into_parts();
        let Response {
            status,
            version,
            headers,
            extensions,
            body,
        } = self;
        parts.status = status;
        parts.version = version;
        parts.headers = headers;
        parts.extensions = extensions;
        http::Response::from_parts(parts, body.stream().into())
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

impl From<Response> for http::Response<hyper::Body> {
    fn from(value: Response) -> Self {
        value.into_resp()
    }
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}
