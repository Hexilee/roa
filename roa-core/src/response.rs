//! A module for Response and its body
use http::{HeaderMap, HeaderValue, StatusCode, Version};
use std::mem::{swap, take};
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
    pub(crate) fn load_resp(&mut self) -> http::Response<hyper::Body> {
        let mut resp = http::Response::new(hyper::Body::empty());
        swap(resp.status_mut(), &mut self.status);
        swap(resp.version_mut(), &mut self.version);
        swap(resp.headers_mut(), &mut self.headers);
        *resp.body_mut() = take(&mut self.body).into();
        resp
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

impl Default for Response {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
