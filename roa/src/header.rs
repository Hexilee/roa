//! The header module of roa.
//! This module provides a Request/Response extension `FriendlyHeaders`.
//!
//! ### When should we use it?
//!
//! You can straightly use raw `http::header::HeaderMap` in roa,
//! but you have to transfer value type between HeaderValue and string then
//! deal with other errors(not `roa::Error`) by yourself.
//! ```rust
//! use roa::{Context, Result, Error};
//! use roa::http::header::{ORIGIN, CONTENT_TYPE};
//! use roa::http::StatusCode;
//!
//! async fn get(mut ctx: Context<()>) -> Result {
//!     if let Some(value) = ctx.req().headers.get(ORIGIN) {
//!         // handle `ToStrError`
//!         let origin = value.to_str().map_err(|_err| Error::new(StatusCode::BAD_REQUEST, "", true))?;
//!         println!("origin: {}", origin);
//!     }
//!     // handle `InvalidHeaderValue`
//!     ctx.resp_mut()
//!        .headers
//!        .insert(
//!            CONTENT_TYPE,
//!            "text/plain".parse().map_err(|_err| Error::new(StatusCode::BAD_REQUEST, "", true))?
//!        );
//!     Ok(())
//! }
//! ```
//!
//! Dealing with errors is necessary but sometimes can be annoying
//!
//! If you are finding some simpler methods to deal with header value, `FriendlyHeaders` is suit for you.
//!
//! ```rust
//! use roa::{Context, Result};
//! use roa::http::header::{ORIGIN, CONTENT_TYPE};
//! use roa::http::StatusCode;
//! use roa::header::FriendlyHeaders;
//!
//! async fn get(mut ctx: Context<()>) -> Result {
//!     println!("origin: {}", ctx.req().must_get(ORIGIN)?);
//!     ctx.resp_mut()
//!        .insert(CONTENT_TYPE, "text/plain")?;
//!     Ok(())
//! }
//! ```
use crate::http::header::{
    AsHeaderName, HeaderMap, HeaderValue, IntoHeaderName, InvalidHeaderValue, ToStrError,
};
use crate::http::StatusCode;
use crate::{Error, Request, Response, Result};

fn handle_invalid_header_value(err: InvalidHeaderValue, value: &str) -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{}\n{} is not a valid header value", err, value),
        false,
    )
}

/// A Request/Response extension.
pub trait FriendlyHeaders {
    /// General error code should be returned when some errors occur.
    ///
    /// 400 BAD REQUEST for Request,
    /// 500 INTERNAL SERVER ERROR for Response.
    const GENERAL_ERROR_CODE: StatusCode;

    /// Get immutable reference of raw header map.
    fn raw_header_map(&self) -> &HeaderMap<HeaderValue>;

    /// Get mutable reference of raw header map.
    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue>;

    /// Deal with `ToStrError`, usually invoked when a header value is gotten,
    /// then fails to be transferred to string.
    /// Throw `Self::GENERAL_ERROR_CODE`.
    fn handle_to_str_error(err: ToStrError, value: &HeaderValue) -> Error {
        Error::new(
            Self::GENERAL_ERROR_CODE,
            format!("{}\n{:?} is not a valid string", err, value),
            true,
        )
    }

    /// Deal with None, usually invoked when a header value is not gotten.
    /// Throw `Self::GENERAL_ERROR_CODE`.
    fn handle_none<K>(key: K) -> Error
    where
        K: AsHeaderName + AsRef<str>,
    {
        Error::new(
            Self::GENERAL_ERROR_CODE,
            format!("header `{}` is required", key.as_ref()),
            true,
        )
    }

    /// Try to get a header value, return None if not exists.
    /// Return Some(Err) if fails to string.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::http::header::{ORIGIN, CONTENT_TYPE};
    /// use roa::http::StatusCode;
    /// use roa::header::FriendlyHeaders;
    ///
    /// async fn get(ctx: Context<()>) -> Result {
    ///     if let Some(value) = ctx.req().get(ORIGIN) {
    ///         println!("origin: {}", value?);     
    ///     }   
    ///     Ok(())
    /// }
    /// ```
    fn get<K>(&self, key: K) -> Option<Result<&str>>
    where
        K: AsHeaderName + AsRef<str>,
    {
        self.raw_header_map().get(key).map(|value| {
            value
                .to_str()
                .map_err(|err| Self::handle_to_str_error(err, value))
        })
    }

    /// Get a header value.
    /// Return Err if not exists or fails to string.
    /// ### Example
    ///
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::http::header::{ORIGIN, CONTENT_TYPE};
    /// use roa::http::StatusCode;
    /// use roa::header::FriendlyHeaders;
    ///
    /// async fn get(ctx: Context<()>) -> Result {
    ///     println!("origin: {}", ctx.req().must_get(ORIGIN)?);     
    ///     Ok(())
    /// }
    /// ```
    fn must_get<K>(&self, key: K) -> Result<&str>
    where
        K: AsHeaderName + AsRef<str>,
    {
        match self.get(key.as_ref()) {
            Some(result) => result,
            None => Err(Self::handle_none(key)),
        }
    }

    /// Get all header value with the same header name.
    /// Return Err if one of them fails to string.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::http::header::{ORIGIN, CONTENT_TYPE};
    /// use roa::http::StatusCode;
    /// use roa::header::FriendlyHeaders;
    ///
    /// async fn get(ctx: Context<()>) -> Result {
    ///     for value in ctx.req().get_all(ORIGIN)?.into_iter() {
    ///         println!("origin: {}", value);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    fn get_all<K>(&self, key: K) -> Result<Vec<&str>>
    where
        K: AsHeaderName,
    {
        let mut ret = Vec::new();
        for value in self.raw_header_map().get_all(key).iter() {
            ret.push(
                value
                    .to_str()
                    .map_err(|err| Self::handle_to_str_error(err, value))?,
            );
        }
        Ok(ret)
    }

    /// Insert a header pair.
    ///
    /// - Return `Err(500 INTERNAL SERVER ERROR)` if value fails to header value.
    /// - Return `Ok(Some(old_value))` if header name already exists.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::http::header::{ORIGIN, CONTENT_TYPE};
    /// use roa::http::StatusCode;
    /// use roa::header::FriendlyHeaders;
    ///
    /// async fn get(mut ctx: Context<()>) -> Result {
    ///     ctx.resp_mut().insert(CONTENT_TYPE, "text/plain")?;   
    ///     Ok(())
    /// }
    /// ```
    fn insert<K, V>(&mut self, key: K, val: V) -> Result<Option<String>>
    where
        K: IntoHeaderName,
        V: AsRef<str>,
    {
        let old_value = self.raw_mut_header_map().insert(
            key,
            val.as_ref()
                .parse()
                .map_err(|err| handle_invalid_header_value(err, val.as_ref()))?,
        );
        Ok(match old_value {
            Some(value) => Some(
                value
                    .to_str()
                    .map(ToString::to_string)
                    .map_err(|err| Self::handle_to_str_error(err, &value))?,
            ),
            None => None,
        })
    }

    /// Append a header pair.
    ///
    /// - Return `Err(500 INTERNAL SERVER ERROR)` if value fails to header value.
    /// - Return `Ok(true)` if header name already exists.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa::{Context, Result};
    /// use roa::http::header::SET_COOKIE;
    /// use roa::http::StatusCode;
    /// use roa::header::FriendlyHeaders;
    ///
    /// async fn get(mut ctx: Context<()>) -> Result {
    ///     ctx.resp_mut().append(SET_COOKIE, "this is a cookie")?;   
    ///     Ok(())
    /// }
    /// ```
    fn append<K, V>(&mut self, key: K, val: V) -> Result<bool>
    where
        K: IntoHeaderName,
        V: AsRef<str>,
    {
        Ok(self.raw_mut_header_map().append(
            key,
            val.as_ref()
                .parse()
                .map_err(|err| handle_invalid_header_value(err, val.as_ref()))?,
        ))
    }
}

impl FriendlyHeaders for Request {
    const GENERAL_ERROR_CODE: StatusCode = StatusCode::BAD_REQUEST;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue> {
        &self.headers
    }

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.headers
    }
}

impl FriendlyHeaders for Response {
    const GENERAL_ERROR_CODE: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue> {
        &self.headers
    }

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.headers
    }
}

#[cfg(test)]
mod tests {
    use crate::http::header::CONTENT_TYPE;
    use crate::http::{HeaderValue, StatusCode};
    use crate::preload::*;
    use crate::{App, Request};
    use mime::TEXT_HTML;

    #[test]
    fn request_raw_mut_header_map() {
        let mut request = Request::default();
        request
            .raw_mut_header_map()
            .insert(CONTENT_TYPE, TEXT_HTML.as_ref().parse().unwrap());
        let content_type = request.must_get(CONTENT_TYPE).unwrap();
        assert_eq!(TEXT_HTML.as_ref(), content_type);
    }

    #[test]
    fn request_get_non_string() {
        let mut request = Request::default();
        request.raw_mut_header_map().insert(
            CONTENT_TYPE,
            HeaderValue::from_bytes([230].as_ref()).unwrap(),
        );
        let ret = request.get(CONTENT_TYPE).unwrap();
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("is not a valid string"));
    }

    #[test]
    fn must_get_fails() {
        let request = Request::default();
        let ret = request.must_get(CONTENT_TYPE);
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert_eq!("header `content-type` is required", status.message);
    }

    #[test]
    fn request_get_all_non_string() {
        let mut request = Request::default();
        request.raw_mut_header_map().insert(
            CONTENT_TYPE,
            HeaderValue::from_bytes([230].as_ref()).unwrap(),
        );
        let ret = request.get_all(CONTENT_TYPE);
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("is not a valid string"));
    }

    #[test]
    fn request_get_all() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::default();
        request.append(CONTENT_TYPE, "text/html")?;
        request.append(CONTENT_TYPE, "text/plain")?;
        let ret = request.get_all(CONTENT_TYPE)?;
        assert_eq!("text/html", ret[0]);
        assert_eq!("text/plain", ret[1]);
        Ok(())
    }

    #[test]
    fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::default();
        request.insert(CONTENT_TYPE, "text/html")?;
        assert_eq!("text/html", request.must_get(CONTENT_TYPE)?);
        let old_data = request.insert(CONTENT_TYPE, "text/plain")?.unwrap();
        assert_eq!("text/html", old_data);
        assert_eq!("text/plain", request.must_get(CONTENT_TYPE)?);
        Ok(())
    }

    #[test]
    fn insert_fail() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::default();
        let ret = request.insert(CONTENT_TYPE, "\r\n");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status.status_code);
        assert!(status.message.ends_with("\r\n is not a valid header value"));
        Ok(())
    }

    #[test]
    fn append_fail() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::default();
        let ret = request.append(CONTENT_TYPE, "\r\n");
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status.status_code);
        assert!(status.message.ends_with("\r\n is not a valid header value"));
        Ok(())
    }
}
