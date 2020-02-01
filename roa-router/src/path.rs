use crate::err::Error;
use crate::Conflict;
use http::uri::PathAndQuery;
use regex::{escape, Regex};
use std::collections::HashSet;
use std::str::FromStr;

#[derive(Clone)]
pub enum Path {
    Static(String),
    Dynamic(RegexPath),
}

#[derive(Clone)]
pub struct RegexPath {
    pub raw: String,
    pub keys: HashSet<String>,
    pub re: Regex,
}

impl Path {
    pub fn raw(&self) -> &str {
        match self {
            Path::Static(ref path) => path.as_str(),
            Path::Dynamic(ref re) => re.raw.as_str(),
        }
    }
}

impl FromStr for Path {
    type Err = Error;
    fn from_str(raw_path: &str) -> Result<Self, Self::Err> {
        let path_and_query = raw_path.parse::<PathAndQuery>()?;
        let path = path_and_query.path().trim_matches('/');
        let (pattern, keys) = path_to_regexp(path)?;
        Ok(if keys.is_empty() {
            Path::Static(path.to_owned())
        } else {
            Path::Dynamic(RegexPath {
                raw: path.to_owned(),
                keys,
                re: Regex::new(&pattern).unwrap_or_else(|err| {
                    panic!(format!(
                        r#"{}
                regex pattern {} is invalid, this is a bug of roa-router::parse::parse.
                please report it to https://github.com/Hexilee/roa"#,
                        err, pattern
                    ))
                }),
            })
        })
    }
}

fn path_to_regexp(path: &str) -> Result<(String, HashSet<String>), Conflict> {
    let mut keys = HashSet::new();
    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.starts_with(':') {
            let key = escape(&segment[1..]);
            if !keys.insert(key.clone()) {
                return Err(Conflict::Variable {
                    paths: (path.to_string(), path.to_string()),
                    var_name: key.clone(),
                });
            }
            segments.push(format!(r"(?P<{}>\S+)", &key))
        } else {
            segments.push(escape(segment))
        }
    }
    Ok((segments.join("/"), keys))
}
