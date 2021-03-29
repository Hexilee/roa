//! A module for Response and its body
use std::ops::{Deref, DerefMut};

use http::{HeaderMap, HeaderValue, StatusCode, Version};

pub use crate::Body;

/// Http response type of roa.
pub struct Response {
    /// Status code.
    pub status: StatusCode,

    /// Version of HTTP protocol.
    pub version: Version,

    /// Raw header map.
    pub headers: HeaderMap<HeaderValue>,

    /// Response body.
    pub body: Body,
}

impl Response {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            body: Body::default(),
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
        http::Response::from_parts(parts, body.into())
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
