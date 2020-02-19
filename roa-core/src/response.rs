//! A module for Response and its body
mod body;
use body::Body;
use bytes::Bytes;
use futures::Stream;
use http::{HeaderValue, StatusCode, Version};
use hyper::HeaderMap;
use std::io::Error;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

type BoxStream =
    Pin<Box<dyn 'static + Send + Sync + Stream<Item = Result<Bytes, Error>>>>;

trait StreamMapper {
    fn map(self: Box<Self>, stream: BoxStream) -> BoxStream;
}

impl<F, S> StreamMapper for F
where
    F: 'static + FnOnce(BoxStream) -> S,
    S: 'static + Send + Sync + Stream<Item = Result<Bytes, Error>>,
{
    fn map(self: Box<Self>, stream: BoxStream) -> BoxStream {
        Box::pin(self(stream))
    }
}

/// Http response type of roa.
pub struct Response {
    /// Status code.
    pub status: StatusCode,

    /// Version of HTTP protocol.
    pub version: Version,

    /// Raw header map.
    pub headers: HeaderMap<HeaderValue>,

    body: Body,

    stream_mapper: Vec<Box<dyn StreamMapper>>,
}

impl Response {
    pub(crate) fn new() -> Self {
        Self {
            status: StatusCode::default(),
            version: Version::default(),
            headers: HeaderMap::default(),
            body: Body::new(),
            stream_mapper: Vec::new(),
        }
    }

    /// Register a body mapper to process body stream.
    pub fn map_body<F, S>(&mut self, mapper: F)
    where
        F: 'static + FnOnce(BoxStream) -> S,
        S: 'static + Send + Sync + Stream<Item = Result<Bytes, Error>>,
    {
        self.stream_mapper.push(Box::new(mapper))
    }

    fn into_resp(self) -> http::Response<hyper::Body> {
        let (mut parts, _) = http::Response::new(()).into_parts();
        let Response {
            status,
            version,
            headers,
            body,
            stream_mapper,
        } = self;
        parts.status = status;
        parts.version = version;
        parts.headers = headers;
        let mut stream: BoxStream = Box::pin(body.into_stream());
        for mapper in stream_mapper {
            stream = mapper.map(stream)
        }
        http::Response::from_parts(parts, hyper::Body::wrap_stream(stream))
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
