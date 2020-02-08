use crate::Body;
use http::{HeaderValue, StatusCode, Version};
use hyper::HeaderMap;
use std::ops::{Deref, DerefMut};

/// Http response type of roa.
pub struct Response {
    pub status: StatusCode,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    body: Body,
}

impl Response {
    pub(crate) fn new() -> Self {
        Self {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            body: Body::new(),
        }
    }

    fn into_resp(self) -> http::Response<hyper::Body> {
        let (mut parts, _) = http::Response::new(()).into_parts();
        let Response {
            status,
            version,
            headers,
            body,
        } = self;
        parts.status = status;
        parts.version = version;
        parts.headers = headers;
        http::Response::from_parts(parts, body.stream().into())
    }
}

impl Deref for Response {
    type Target = Body;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for Response {
    #[inline]
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
