use super::help::bug_report;
use crate::http::header::HeaderValue;
use crate::Status;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::convert::{TryFrom, TryInto};
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

/// A structure to generate value of "Content-Disposition"
pub struct ContentDisposition {
    typ: DispositionType,
    encoded_filename: Option<String>,
}

impl ContentDisposition {
    /// Construct by disposition type and optional filename.
    #[inline]
    pub(crate) fn new(typ: DispositionType, filename: Option<&str>) -> Self {
        Self {
            typ,
            encoded_filename: filename
                .map(|name| utf8_percent_encode(name, HTTP_VALUE).to_string()),
        }
    }
}

impl TryFrom<ContentDisposition> for HeaderValue {
    type Error = Status;
    #[inline]
    fn try_from(value: ContentDisposition) -> Result<Self, Self::Error> {
        value
            .to_string()
            .try_into()
            .map_err(|err| bug_report(format!("{}\nNot a valid header value", err)))
    }
}

impl Display for ContentDisposition {
    #[inline]
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
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DispositionType::Inline => f.write_str("inline"),
            DispositionType::Attachment => f.write_str("attachment"),
        }
    }
}
