use http::uri::InvalidUri;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    InvalidUri(InvalidUri),
    Conflict(Conflict),
}

#[derive(Debug)]
pub enum Conflict {
    Path(String),
    Method(String, http::Method),
    Variable {
        paths: (String, String),
        var_name: String,
    },
}

impl Display for Conflict {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Conflict::Path(path) => f.write_str(&format!("conflict path: `{}`", path)),
            Conflict::Method(path, method) => f.write_str(&format!(
                "conflict method: `{}` on `{}` is already set",
                method, path
            )),
            Conflict::Variable { paths, var_name } => f.write_str(&format!(
                "conflict variable `{}`: between `{}` and `{}`",
                var_name, paths.0, paths.1
            )),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Error::Conflict(conflict) => f.write_str(&format!("Conflict! {}", conflict)),
            Error::InvalidUri(invalid) => f.write_str(&format!("Invalid Uri! {}", invalid)),
        }
    }
}

impl From<Conflict> for Error {
    fn from(conflict: Conflict) -> Self {
        Error::Conflict(conflict)
    }
}

impl From<InvalidUri> for Error {
    fn from(invalid: InvalidUri) -> Self {
        Error::InvalidUri(invalid)
    }
}

impl std::error::Error for Conflict {}
impl std::error::Error for Error {}
