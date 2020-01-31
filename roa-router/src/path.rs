use http::uri::{InvalidUri, PathAndQuery};
use regex::{escape, Regex};
use std::str::FromStr;

#[derive(Clone)]
pub enum Path {
    Static(String),
    Dynamic(RegexPath),
}

#[derive(Clone)]
pub struct RegexPath {
    pub keys: Vec<String>,
    pub re: Regex,
}

impl FromStr for Path {
    type Err = InvalidUri;
    fn from_str(raw_path: &str) -> Result<Self, Self::Err> {
        let path_and_query = raw_path.parse::<PathAndQuery>()?;
        let path = path_and_query.path().trim_matches('/');
        let (pattern, keys) = path_to_regexp(path);
        Ok(if keys.is_empty() {
            Path::Static(path.to_owned())
        } else {
            Path::Dynamic(RegexPath {
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

fn path_to_regexp(path: &str) -> (String, Vec<String>) {
    let mut keys = vec![];
    let pattern = path
        .split('/')
        .map(|segment: &str| {
            if segment.starts_with(':') {
                let key = escape(&segment[1..]);
                keys.push(key.clone());
                format!(r"(?P<{}>\w+)", &key)
            } else {
                escape(segment)
            }
        })
        .collect::<Vec<String>>()
        .join("/");
    (pattern, keys)
}
