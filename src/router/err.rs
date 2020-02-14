use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub enum RouterError {
    MissingVariable(String),
    Conflict(Conflict),
}

#[derive(Debug, Eq, PartialEq)]
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

impl Display for RouterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            RouterError::Conflict(conflict) => {
                f.write_str(&format!("Conflict! {}", conflict))
            }
            RouterError::MissingVariable(path) => {
                f.write_str(&format!("missing variable on path {}", path))
            }
        }
    }
}

impl From<Conflict> for RouterError {
    fn from(conflict: Conflict) -> Self {
        RouterError::Conflict(conflict)
    }
}

impl std::error::Error for Conflict {}
impl std::error::Error for RouterError {}

#[cfg(test)]
mod tests {
    use super::{Conflict, RouterError};

    #[test]
    fn conflict_to_string() {
        assert_eq!(
            "conflict path: `/`",
            Conflict::Path("/".to_string()).to_string()
        );
        assert_eq!(
            "conflict method: `GET` on `/` is already set",
            Conflict::Method("/".to_string(), http::Method::GET).to_string()
        );
        assert_eq!(
            "conflict variable `id`: between `/:id` and `/user/:id`",
            Conflict::Variable {
                paths: ("/:id".to_string(), "/user/:id".to_string()),
                var_name: "id".to_string()
            }
            .to_string()
        );
    }

    #[test]
    fn err_to_string() {
        assert_eq!(
            "Conflict! conflict path: `/`",
            RouterError::Conflict(Conflict::Path("/".to_string())).to_string()
        );
        assert_eq!(
            "missing variable on path /:",
            RouterError::MissingVariable("/:".to_string()).to_string()
        );
    }
}
