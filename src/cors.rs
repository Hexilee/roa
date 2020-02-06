use crate::{Context, DynTargetHandler, Model, Next, TargetHandler};
use http::header::{
    HeaderName, HeaderValue, ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
    ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
    ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
    VARY,
};
use http::Method;
use http::StatusCode;

#[derive(Debug, Clone)]
pub struct Options {
    pub allow_origin: Option<String>,
    pub allow_methods: Vec<Method>,
    pub expose_headers: Vec<HeaderName>,
    pub allow_headers: Vec<HeaderName>,
    pub max_age: u64,
    pub credentials: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_origin: None,
            allow_methods: vec![
                Method::GET,
                Method::HEAD,
                Method::PUT,
                Method::POST,
                Method::DELETE,
                Method::PATCH,
            ],
            expose_headers: Vec::new(),
            allow_headers: Vec::new(),
            max_age: 86400,
            credentials: true,
        }
    }
}

const BUG_HELP: &str = r"
This is a bug of crate `roa` or `http`. 
Please report it to https://github.com/Hexilee/roa";

impl Options {
    fn join_methods(&self) -> HeaderValue {
        self.allow_methods
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<String>>()
            .join(", ")
            .parse()
            .expect(BUG_HELP)
    }

    fn join_expose_headers(&self) -> HeaderValue {
        self.expose_headers
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
            .parse()
            .expect(BUG_HELP)
    }

    fn join_allow_headers(&self) -> HeaderValue {
        self.allow_headers
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
            .parse()
            .expect(BUG_HELP)
    }

    fn parse_max_age(&self) -> HeaderValue {
        self.max_age.to_string().parse().expect(BUG_HELP)
    }
}

pub fn cors<M: Model>(options: Options) -> Box<DynTargetHandler<M, Next>> {
    // Parse `allow_origin` to HeaderValue if set.
    let options_allow_origin = options.allow_origin.clone().map(|origin| {
        HeaderValue::from_str(&origin)
            .unwrap_or_else(|err| panic!("{}\nallow_origin is not a valid HeaderValue", err))
    });
    let allow_headers = options.join_allow_headers();
    let allow_methods = options.join_methods();
    let expose_headers = options.join_expose_headers();
    let max_age = options.parse_max_age();
    let credentials = options.credentials;

    Box::new(move |ctx: Context<M>, next: Next| {
        let options_allow_origin = options_allow_origin.clone();
        let mut allow_headers = allow_headers.clone();
        let allow_methods = allow_methods.clone();
        let expose_headers = expose_headers.clone();
        let max_age = max_age.clone();
        async move {
            // Always set Vary header
            // https://github.com/rs/cors/issues/10
            ctx.resp_mut()
                .await
                .headers
                .append(VARY, HeaderValue::from_name(ORIGIN));

            // If `Origin` header not set, skip this middleware.
            let req_origin = match ctx.header_value(&ORIGIN).await {
                Some(origin) => origin,
                None => return next().await,
            };

            // If Options::allow_origin is None, `Access-Control-Allow-Origin` will be set to `Origin`.
            let allow_origin = match options_allow_origin {
                Some(origin) => origin,
                None => req_origin,
            };

            if ctx.method().await != Method::OPTIONS {
                // Simple Request

                // set "Access-Control-Allow-Origin"
                ctx.resp_mut()
                    .await
                    .headers
                    .insert(ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin);

                // Try to set "Access-Control-Allow-Credentials"
                if credentials {
                    ctx.resp_mut().await.headers.insert(
                        ACCESS_CONTROL_ALLOW_CREDENTIALS,
                        HeaderValue::from_static("true"),
                    );
                }

                // set "Access-Control-Expose-Headers"
                if !expose_headers.is_empty() {
                    ctx.resp_mut()
                        .await
                        .headers
                        .insert(ACCESS_CONTROL_EXPOSE_HEADERS, expose_headers);
                }
                next().await
            } else {
                // Preflight Request

                // If there is no Access-Control-Request-Method header or if parsing failed,
                // do not set any additional headers and terminate this set of steps.
                // The request is outside the scope of this specification.
                if !ctx
                    .req()
                    .await
                    .headers
                    .contains_key(&ACCESS_CONTROL_REQUEST_METHOD)
                {
                    // this not preflight request, ignore it
                    return next().await;
                }

                // set "Access-Control-Allow-Origin"
                ctx.resp_mut()
                    .await
                    .headers
                    .insert(ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin);

                // Try to set "Access-Control-Allow-Credentials"
                if credentials {
                    ctx.resp_mut().await.headers.insert(
                        ACCESS_CONTROL_ALLOW_CREDENTIALS,
                        HeaderValue::from_static("true"),
                    );
                }

                // set "Access-Control-Max-Age"
                ctx.resp_mut()
                    .await
                    .headers
                    .insert(ACCESS_CONTROL_MAX_AGE, max_age);

                // Try to set "Access-Control-Allow-Methods"
                if !allow_methods.is_empty() {
                    ctx.resp_mut()
                        .await
                        .headers
                        .insert(ACCESS_CONTROL_ALLOW_METHODS, allow_methods);
                }

                // If allow_headers is None, try to set `Access-Control-Allow-Headers` as `Access-Control-Request-Headers`.
                if allow_headers.is_empty() {
                    if let Some(value) = ctx.header_value(&ACCESS_CONTROL_REQUEST_HEADERS).await {
                        allow_headers = value
                    }
                }

                // Try to set "Access-Control-Allow-Methods"
                if !allow_headers.is_empty() {
                    ctx.resp_mut()
                        .await
                        .headers
                        .insert(ACCESS_CONTROL_ALLOW_HEADERS, allow_headers);
                }

                ctx.resp_mut().await.status = StatusCode::NO_CONTENT;
                Ok(())
            }
        }
    })
    .dynamic()
}

#[cfg(test)]
mod tests {
    use super::{cors, Options};
    use crate::preload::*;
    use crate::App;
    use async_std::task::spawn;
    use http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, ORIGIN};
    use http::StatusCode;

    #[tokio::test]
    async fn cors_default() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        let (addr, server) = app
            .gate(cors(Options::default()))
            .gate(|ctx, _next| async move {
                ctx.write_text("Hello, World");
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();

        // No Origin
        let resp = client.get(&format!("http://{}", addr)).send().await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        Ok(())
    }
}
