use crate::bug_report;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use roa_core::http::header::HeaderValue;
use roa_core::Result;
use std::fmt::{self, Display, Formatter};

// This encode set is used for HTTP header values and is defined at
// https://tools.ietf.org/html/rfc5987#section-3.2
const HTTP_VALUE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'-')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'{')
    .add(b'}');

/// Type of content-disposition, inline or attachment
#[derive(Clone, Debug, PartialEq)]
pub enum DispositionType {
    /// Inline implies default processing
    Inline,
    /// Attachment implies that the recipient should prompt the user to save the response locally,
    /// rather than process it normally (as per its media type).
    Attachment,
}

pub struct ContentDisposition {
    typ: DispositionType,
    encoded_filename: Option<String>,
}

impl ContentDisposition {
    pub(crate) fn new(typ: DispositionType, filename: Option<&str>) -> Self {
        Self {
            typ,
            encoded_filename: filename
                .map(|name| utf8_percent_encode(name, HTTP_VALUE).to_string()),
        }
    }

    pub fn value(&self) -> Result<HeaderValue> {
        let value_str = self.to_string();
        value_str.parse().map_err(|err| {
            bug_report(format!(
                "{}\n{} is not a valid header value",
                err, value_str
            ))
        })
    }
}

impl Display for ContentDisposition {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.encoded_filename {
            None => f.write_fmt(format_args!("{}", self.typ)),
            Some(name) => f.write_fmt(format_args!(
                "{}; filename={}; filename*=UTF-8''{}",
                self.typ, name, name
            )),
        }
    }
}

impl Display for DispositionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DispositionType::Inline => f.write_str("inline"),
            DispositionType::Attachment => f.write_str("attachment"),
        }
    }
}
