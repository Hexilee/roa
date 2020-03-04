//! A module for Response and its body
use http::{HeaderMap, HeaderValue, StatusCode, Version};
use std::ops::{Deref, DerefMut};

pub use crate::Body;

/// Http response type of roa.
pub struct Response {
    /// Status code.
    pub status: StatusCode,

    /// Version of HTTP protocol.
    pub version: Version,

    /// Raw header map.
    pub headers: HeaderMap<HeaderValue>,

    body: Body,
}

impl Response {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            body: Body::new(),
        }
    }

    #[inline]
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
        http::Response::from_parts(parts, hyper::Body::wrap_stream(body))
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
    #[inline]
    fn from(value: Response) -> Self {
        value.into_resp()
    }
}

impl Default for Response {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
