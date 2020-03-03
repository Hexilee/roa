use super::{Conflict, RouterError};
use regex::{escape, Captures, Regex};
use std::collections::HashSet;
use std::convert::AsRef;
use std::str::FromStr;

/// Match pattern *{variable}
const WILDCARD: &str = r"\*\{(?P<var>\w*)\}";

/// Match pattern /:variable/
const VARIABLE: &str = r"/:(?P<var>\w*)/";

/// {/path path/ /path/} => /path/
pub fn standardize_path(raw_path: &str) -> String {
    format!("/{}/", raw_path.trim_matches('/'))
}

/// Join multiple segments.
pub fn join_path<'a>(paths: impl 'a + AsRef<[&'a str]>) -> String {
    paths
        .as_ref()
        .iter()
        .map(|path| path.trim_matches('/'))
        .filter(|path| !path.is_empty())
        .collect::<Vec<&str>>()
        .join("/")
}

/// Build pattern.
fn must_build(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|err| {
        panic!(
            r#"{}
                regex pattern {} is invalid, this is a bug of roa-router::path.
                please report it to https://github.com/Hexilee/roa"#,
            err, pattern
        )
    })
}

/// Parsed path.
#[derive(Clone)]
pub enum Path {
    Static(String),
    Dynamic(RegexPath),
}

/// Dynamic path.
#[derive(Clone)]
pub struct RegexPath {
    pub raw: String,
    pub vars: HashSet<String>,
    pub re: Regex,
}

impl FromStr for Path {
    type Err = RouterError;
    fn from_str(raw_path: &str) -> Result<Self, Self::Err> {
        let path = standardize_path(raw_path);
        Ok(match path_to_regexp(&path)? {
            None => Path::Static(path),
            Some((pattern, vars)) => Path::Dynamic(RegexPath {
                raw: path,
                vars,
                re: must_build(&format!(r"^{}$", pattern)),
            }),
        })
    }
}

fn path_to_regexp(path: &str) -> Result<Option<(String, HashSet<String>)>, RouterError> {
    let mut pattern = escape(path);
    let mut vars = HashSet::new();
    let wildcard_re = must_build(WILDCARD);
    let variable_re = must_build(VARIABLE);
    let wildcards: Vec<Captures> = wildcard_re.captures_iter(path).collect();
    let variable_template = path.replace('/', "//"); // to match continuous variables like /:year/:month/:day/
    let variables: Vec<Captures> =
        variable_re.captures_iter(&variable_template).collect();
    if wildcards.is_empty() && variables.is_empty() {
        Ok(None)
    } else {
        // detect variable conflicts.
        let try_add_variable = |set: &mut HashSet<String>, variable: String| {
            if set.insert(variable.clone()) {
                Ok(())
            } else {
                Err(Conflict::Variable {
                    paths: (path.to_string(), path.to_string()),
                    var_name: variable,
                })
            }
        };

        // match wildcard patterns
        for cap in wildcards {
            let variable = &cap["var"];
            if variable == r"" {
                return Err(RouterError::MissingVariable(path.to_string()));
            }
            let var = escape(variable);
            pattern = pattern.replace(
                &escape(&format!(r"*{{{}}}", variable)),
                &format!(r"(?P<{}>\S+)", &var),
            );
            try_add_variable(&mut vars, var)?;
        }

        // match segment variable patterns
        for cap in variables {
            let variable = &cap["var"];
            if variable == "" {
                return Err(RouterError::MissingVariable(path.to_string()));
            }
            let var = escape(variable);
            pattern = pattern.replace(
                &escape(&format!(r":{}", variable)),
                &format!(r"(?P<{}>[^\s/]+)", &var),
            );
            try_add_variable(&mut vars, var)?;
        }
        Ok(Some((pattern, vars)))
    }
}

#[cfg(test)]
mod tests {
    use super::Path;
    use super::{must_build, path_to_regexp, VARIABLE, WILDCARD};
    use test_case::test_case;

    #[test_case("/:id/"; "pure dynamic")]
    #[test_case("/user/:id/"; "static prefix")]
    #[test_case("/user/:id/name"; "static prefix and suffix")]
    fn var_regex_match(path: &str) {
        let re = must_build(VARIABLE);
        let cap = re.captures(path);
        assert!(cap.is_some());
        assert_eq!("id", &cap.unwrap()["var"]);
    }

    #[test_case("/-:id/"; "invalid prefix")]
    #[test_case("/:i-d/"; "invalid variable name")]
    #[test_case("/:id-/"; "invalid suffix")]
    fn var_regex_mismatch(path: &str) {
        let re = must_build(VARIABLE);
        let cap = re.captures(path);
        assert!(cap.is_none());
    }

    #[test_case("*{id}"; "pure dynamic")]
    #[test_case("user-*{id}"; "static prefix")]
    #[test_case("user-*{id}-name"; "static prefix and suffix")]
    fn wildcard_regex_match(path: &str) {
        let re = must_build(WILDCARD);
        let cap = re.captures(path);
        assert!(cap.is_some());
        assert_eq!("id", &cap.unwrap()["var"]);
    }

    #[test_case("*"; "no variable")]
    #[test_case("*{-id}"; "invalid variable name")]
    fn wildcard_regex_mismatch(path: &str) {
        let re = must_build(WILDCARD);
        let cap = re.captures(path);
        assert!(cap.is_none());
    }

    #[test_case(r"/:id/" => r"/(?P<id>[^\s/]+)/"; "single variable")]
    #[test_case(r"/:year/:month/:day/" => r"/(?P<year>[^\s/]+)/(?P<month>[^\s/]+)/(?P<day>[^\s/]+)/"; "multiple variable")]
    #[test_case(r"*{id}" => r"(?P<id>\S+)"; "single wildcard")]
    #[test_case(r"*{year}_*{month}_*{day}" => r"(?P<year>\S+)_(?P<month>\S+)_(?P<day>\S+)"; "multiple wildcard")]
    fn path_to_regexp_dynamic_pattern(path: &str) -> String {
        path_to_regexp(path).unwrap().unwrap().0
    }

    #[test_case(r"/id/")]
    #[test_case(r"/user/post/")]
    fn path_to_regexp_static(path: &str) {
        assert!(path_to_regexp(path).unwrap().is_none())
    }

    #[test_case(r"/:/"; "missing variable name")]
    #[test_case(r"*{}"; "wildcard missing variable name")]
    #[test_case(r"/:id/:id/"; "conflict variable")]
    #[test_case(r"*{id}-*{id}"; "wildcard conflict variable")]
    #[test_case(r"/:id/*{id}"; "mix conflict variable")]
    fn path_to_regexp_err(path: &str) {
        assert!(path_to_regexp(path).is_err())
    }

    fn path_match(pattern: &str, path: &str) {
        let pattern: Path = pattern.parse().unwrap();
        match pattern {
            Path::Static(pattern) => panic!(format!("`{}` should be dynamic", pattern)),
            Path::Dynamic(re) => assert!(re.re.is_match(path)),
        }
    }

    fn path_not_match(pattern: &str, path: &str) {
        let pattern: Path = pattern.parse().unwrap();
        match pattern {
            Path::Static(pattern) => panic!(format!("`{}` should be dynamic", pattern)),
            Path::Dynamic(re) => {
                println!("regex: {}", re.re.to_string());
                assert!(!re.re.is_match(path))
            }
        }
    }

    #[test_case(r"/user/1/")]
    #[test_case(r"/user/65535/")]
    fn single_variable_path_match(path: &str) {
        path_match(r"/user/:id", path)
    }

    #[test_case(r"/2000/01/01/")]
    #[test_case(r"/2020/02/20/")]
    fn multiple_variable_path_match(path: &str) {
        path_match(r"/:year/:month/:day", path)
    }

    #[test_case(r"/usr/include/boost/boost.h/")]
    #[test_case(r"/usr/include/uv/uv.h/")]
    fn segment_wildcard_path_match(path: &str) {
        path_match(r"/usr/include/*{dir}/*{file}.h", path)
    }

    #[test_case(r"/srv/static/app/index.html/")]
    #[test_case(r"/srv/static/../../index.html/")]
    fn full_wildcard_path_match(path: &str) {
        path_match(r"/srv/static/*{path}/", path)
    }

    #[test_case(r"/srv/app/index.html/")]
    #[test_case(r"/srv/../../index.html/")]
    fn variable_path_not_match(path: &str) {
        path_not_match(r"/srv/:path/", path)
    }

    #[should_panic]
    #[test]
    fn must_build_fails() {
        must_build(r"{");
    }
}
