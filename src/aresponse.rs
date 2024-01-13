use std::time::Instant;

use rouille::Response;


pub struct AResponse {
    pub response: Response,
    pub sleep_until: Option<Instant>,
}

impl From<Response> for AResponse {
    fn from(response: Response) -> Self {
        Self {
            response,
            sleep_until: None
        }
    }
}

pub trait ToAResponse {
    fn to_aresponse(self, sleep_until: Option<Instant>) -> AResponse;
}

impl ToAResponse for Response {
    fn to_aresponse(self, sleep_until: Option<Instant>) -> AResponse {
        AResponse { response: self, sleep_until }
    }
}
