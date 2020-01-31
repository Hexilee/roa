//use http::uri::PathAndQuery;
//use proc_macro2::{Span, TokenStream};
//use quote::quote;
//use regex::{escape, Regex};
//use syn::{Error, LitStr};
//
//pub fn parse(path_lit: LitStr) -> Result<TokenStream, Error> {
//    let path_and_query = path_lit
//        .value()
//        .parse::<PathAndQuery>()
//        .map_err(|err| Error::new(path_lit.span(), format!("{}\npath invalid", err)))?;
//    let path = path_and_query.path().trim_matches('/');
//    let (pattern, keys) = path_to_regexp(path);
//    check_pattern(&pattern)?;
//    Ok(if keys.is_empty() {
//        quote!(#path_lit)
//    } else {
//        let mut keys_token = quote!();
//        for key in keys {
//            keys_token = quote!(#keys_token#key, )
//        }
//        quote!((#pattern, &[#keys_token][..]))
//    })
//}
//
//fn path_to_regexp(path: &str) -> (String, Vec<String>) {
//    let mut keys = vec![];
//    let pattern = path
//        .split('/')
//        .map(|segment: &str| {
//            if segment.starts_with(':') {
//                let key = escape(&segment[1..]);
//                keys.push(key.clone());
//                format!(r"(?P<{}>\w+)", &key)
//            } else {
//                escape(segment)
//            }
//        })
//        .collect::<Vec<String>>()
//        .join("/");
//    (pattern, keys)
//}
//
//fn check_pattern(pattern: &str) -> Result<Regex, Error> {
//    Regex::new(pattern).map_err(|err| {
//        Error::new(
//            Span::call_site(),
//            format!(
//                r#"{}
//                regex pattern {} is invalid, this is a bug of roa-proc-macro::dynamic_path.
//                please report it to https://github.com/Hexilee/roa"#,
//                err, pattern
//            ),
//        )
//    })
//}
