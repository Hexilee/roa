use crate::{Request, Response, Status};
use http::header::{
    AsHeaderName, HeaderMap, HeaderValue, IntoHeaderName, InvalidHeaderValue, ToStrError,
};
use http::StatusCode;

fn handle_invalid_header_value(err: InvalidHeaderValue, value: &str) -> Status {
    Status::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("{}\n{} is not a valid header value", err, value),
        false,
    )
}

pub trait StringHeaders {
    const GENERAL_ERROR_CODE: StatusCode;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue>;

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue>;

    fn handle_to_str_error(err: ToStrError, value: &HeaderValue) -> Status {
        Status::new(
            Self::GENERAL_ERROR_CODE,
            format!("{}\n{:?} is not a valid string", err, value),
            true,
        )
    }

    fn handle_none<K>(key: K) -> Status
    where
        K: AsHeaderName + AsRef<str>,
    {
        Status::new(
            Self::GENERAL_ERROR_CODE,
            format!("header `{}` is required", key.as_ref()),
            true,
        )
    }

    fn get<K>(&self, key: K) -> Option<Result<&str, Status>>
    where
        K: AsHeaderName + AsRef<str>,
    {
        self.raw_header_map().get(key).map(|value| {
            value
                .to_str()
                .map_err(|err| Self::handle_to_str_error(err, value))
        })
    }

    fn must_get<K>(&self, key: K) -> Result<&str, Status>
    where
        K: AsHeaderName + AsRef<str>,
    {
        match self.get(key.as_ref()) {
            Some(result) => result,
            None => Err(Self::handle_none(key)),
        }
    }

    fn get_all<K>(&self, key: K) -> Result<Vec<&str>, Status>
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

    fn insert<K, V>(&mut self, key: K, val: V) -> Result<Option<String>, Status>
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

    fn append<K, V>(&mut self, key: K, val: V) -> Result<bool, Status>
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

impl StringHeaders for Request {
    const GENERAL_ERROR_CODE: StatusCode = StatusCode::BAD_REQUEST;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue> {
        &self.headers
    }

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.headers
    }
}

impl StringHeaders for Response {
    const GENERAL_ERROR_CODE: StatusCode = StatusCode::INTERNAL_SERVER_ERROR;

    fn raw_header_map(&self) -> &HeaderMap<HeaderValue> {
        &self.headers
    }

    fn raw_mut_header_map(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.headers
    }
}
