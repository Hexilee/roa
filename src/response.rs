use hyper::Body;

pub struct Response {}

impl Response {
    pub fn new() -> Self {
        Self {}
    }

    pub fn into_resp(self) -> hyper::Response<Body> {
        unimplemented!()
    }
}
