use crate::core::{async_trait, Context, Middleware, Next, Result, State};
use crate::preload::*;
use async_std::sync::Arc;
use http::header::{
    HeaderName, HeaderValue, ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
    ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
    ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
    VARY,
};
use http::Method;
use http::StatusCode;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct Cors {
    #[builder(default)]
    allow_origin: Option<String>,

    #[builder(default = vec![Method::GET, Method::HEAD, Method::PUT, Method::POST, Method::DELETE, Method::PATCH,])]
    allow_methods: Vec<Method>,

    #[builder(default)]
    expose_headers: Vec<HeaderName>,

    #[builder(default)]
    allow_headers: Vec<HeaderName>,

    #[builder(default = 86400)]
    max_age: u64,

    #[builder(default = true)]
    credentials: bool,
}

const BUG_HELP: &str = r"
 This is a bug of crate `roa` or `http`.
 Please report it to https://github.com/Hexilee/roa";

impl Cors {
    fn join_methods(&self) -> String {
        self.allow_methods
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn join_expose_headers(&self) -> String {
        self.expose_headers
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
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

    async fn if_continue<S: State>(&self, ctx: &Context<S>) -> bool {
        let method = ctx.method().await;
        let headers = &ctx.req().await.headers;
        // If there is no Origin header or if parsing failed, skip this middleware.
        headers.contains_key(ORIGIN)
            // If method is OPTIONS and there is no Access-Control-Request-Method header or if parsing failed,
            // do not set any additional headers and terminate this set of steps.
            // The request is outside the scope of this specification.
            && (method != Method::OPTIONS || headers.contains_key(ACCESS_CONTROL_REQUEST_METHOD))
    }
}

#[async_trait]
impl<S: State> Middleware<S> for Cors {
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result {
        // Always set Vary header
        // https://github.com/rs/cors/issues/10
        ctx.resp_mut().await.append(VARY, ORIGIN)?;

        if !self.if_continue(&ctx).await {
            return next().await;
        }

        // If Options::allow_origin is None, `Access-Control-Allow-Origin` will be set to `Origin`.
        let allow_origin = match self.allow_origin {
            Some(ref origin) => origin.clone(),
            None => ctx.req().await.get(ORIGIN).expect(BUG_HELP)?.to_owned(),
        };

        // Set "Access-Control-Allow-Origin"
        ctx.resp_mut()
            .await
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin)?;

        // Try to set "Access-Control-Allow-Credentials"
        if self.credentials {
            ctx.resp_mut()
                .await
                .insert(ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")?;
        }

        if ctx.method().await != Method::OPTIONS {
            // Simple Request
            // Set "Access-Control-Expose-Headers"
            if !self.expose_headers.is_empty() {
                ctx.resp_mut()
                    .await
                    .insert(ACCESS_CONTROL_EXPOSE_HEADERS, self.join_expose_headers())?;
            }
            next().await
        } else {
            // Preflight Request
            // Set "Access-Control-Max-Age"
            ctx.resp_mut()
                .await
                .insert(ACCESS_CONTROL_MAX_AGE, self.max_age.to_string())?;

            // Try to set "Access-Control-Allow-Methods"
            if !self.allow_methods.is_empty() {
                ctx.resp_mut()
                    .await
                    .insert(ACCESS_CONTROL_ALLOW_METHODS, self.join_methods())?;
            }

            // If allow_headers is None, try to assign `Access-Control-Request-Headers` to `Access-Control-Allow-Headers`.
            let mut allow_headers = self.join_allow_headers();
            if allow_headers.is_empty() {
                if let Some(value) = ctx.header_value(ACCESS_CONTROL_REQUEST_HEADERS).await {
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
}

#[cfg(test)]
mod tests {
    use super::Cors;
    use crate::core::App;
    use crate::preload::*;
    use async_std::task::spawn;
    use http::header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
        ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
        CONTENT_TYPE, ORIGIN, VARY,
    };
    use http::{HeaderValue, StatusCode};

    #[tokio::test]
    async fn default_cors() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        let (addr, server) = app
            .gate(Cors::builder().build())
            .end(|ctx| async move {
                ctx.write_text("Hello, World").await?;
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();

        // No origin
        let resp = client.get(&format!("http://{}", addr)).send().await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // simple request
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ORIGIN, "github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!(
            "github.com",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()?
        );
        assert_eq!(
            "true",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap()
                .to_str()?
        );
        assert!(resp.headers().get(ACCESS_CONTROL_EXPOSE_HEADERS).is_none());
        assert_eq!("Hello, World", resp.text().await?);

        // options, no Access-Control-Request-Method
        let resp = client
            .request(http::Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // options, contains Access-Control-Request-Method
        let resp = client
            .request(http::Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "github.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "GET, POST")
            .header(
                ACCESS_CONTROL_REQUEST_HEADERS,
                HeaderValue::from_name(CONTENT_TYPE),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::NO_CONTENT, resp.status());
        assert_eq!(
            "github.com",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()?
        );
        assert_eq!(
            "true",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap()
                .to_str()?
        );

        assert_eq!(
            "86400",
            resp.headers()
                .get(ACCESS_CONTROL_MAX_AGE)
                .unwrap()
                .to_str()?
        );

        assert_eq!(
            "GET, HEAD, PUT, POST, DELETE, PATCH",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_METHODS)
                .unwrap()
                .to_str()?
        );

        assert_eq!(
            HeaderValue::from_name(CONTENT_TYPE),
            resp.headers().get(ACCESS_CONTROL_ALLOW_HEADERS).unwrap()
        );
        assert_eq!("", resp.text().await?);
        //
        Ok(())
    }
}
