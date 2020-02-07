use crate::{Error, Request, Response, Result};
use http::header::{
    AsHeaderName, HeaderMap, HeaderValue, IntoHeaderName, InvalidHeaderValue, ToStrError,
};
use http::StatusCode;

fn handle_invalid_header_value(err: InvalidHeaderValue, value: &str) -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{}\n{} is not a valid header value", err, value),
        false,
    )
}

pub trait FriendlyHeaders {
    const GENERAL_ERROR_CODE: StatusCode;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue>;

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue>;

    fn handle_to_str_error(err: ToStrError, value: &HeaderValue) -> Error {
        Error::new(
            Self::GENERAL_ERROR_CODE,
            format!("{}\n{:?} is not a valid string", err, value),
            true,
        )
    }

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

    fn must_get<K>(&self, key: K) -> Result<&str>
    where
        K: AsHeaderName + AsRef<str>,
    {
        match self.get(key.as_ref()) {
            Some(result) => result,
            None => Err(Self::handle_none(key)),
        }
    }

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
    use super::FriendlyHeaders;
    use crate::Request;
    use http::header::CONTENT_TYPE;
    use http::{HeaderValue, StatusCode};
    use mime::TEXT_HTML;

    #[test]
    fn request_raw_mut_header_map() {
        let mut request = Request::default();
        request
            .raw_mut_header_map()
            .insert(CONTENT_TYPE, TEXT_HTML.as_ref().parse().unwrap());
        let content_type = request.must_get(&CONTENT_TYPE).unwrap();
        assert_eq!(TEXT_HTML.as_ref(), content_type);
    }

    #[test]
    fn request_get_non_string() {
        let mut request = Request::default();
        request.raw_mut_header_map().insert(
            CONTENT_TYPE,
            HeaderValue::from_bytes([230].as_ref()).unwrap(),
        );
        let ret = request.get(&CONTENT_TYPE).unwrap();
        assert!(ret.is_err());
        let status = ret.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status.message.ends_with("is not a valid string"));
    }

    #[test]
    fn must_get_fails() {
        let request = Request::default();
        let ret = request.must_get(&CONTENT_TYPE);
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
        let ret = request.get_all(&CONTENT_TYPE);
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
        let ret = request.get_all(&CONTENT_TYPE)?;
        assert_eq!("text/html", ret[0]);
        assert_eq!("text/plain", ret[1]);
        Ok(())
    }

    #[test]
    fn insert() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::default();
        request.insert(CONTENT_TYPE, "text/html")?;
        assert_eq!("text/html", request.must_get(&CONTENT_TYPE)?);
        let old_data = request.insert(CONTENT_TYPE, "text/plain")?.unwrap();
        assert_eq!("text/html", old_data);
        assert_eq!("text/plain", request.must_get(&CONTENT_TYPE)?);
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
