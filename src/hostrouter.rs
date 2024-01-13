//! Route according to the domain

use std::{sync::{Arc, Mutex}, collections::HashMap};

use kstring::KString;

use crate::{router::MultiRouter,
            handler::Handler,
            apachelog::Logs,
            warn,
            arequest::ARequest,
            ahtml::HtmlAllocator,
            webutils::errorpage_from_status,
            http_response_status_codes::HttpResponseStatusCode, http_request_method::HttpRequestMethodSimple, aresponse::AResponse};

/// Route for a particular host (domain)
pub struct HostRouter {
    pub router: Option<Arc<MultiRouter<Arc<dyn Handler>>>>,
    /// Fallback when no router accepted the path.
    pub fallback: Option<Arc<dyn Handler>>,
    /// Logs when using either routed or fallback handler.
    pub logs: Arc<Mutex<Logs>>,
}

impl HostRouter {
    pub fn handle_request(
        &self,
        request: &ARequest,
        method: HttpRequestMethodSimple,
        allocator: &HtmlAllocator
    ) -> (Arc<Mutex<Logs>>, anyhow::Result<AResponse>)
    {
        if let Some(router) = &self.router {
            if let Some((handlers, rest)) = router.get(request.path()) {
                // dt!("multirouter", rest);
                for handler in handlers {
                    match handler.call(&request, method, &rest, allocator) {
                        Ok(Some(response)) => return (self.logs.clone(), Ok(response)),
                        Ok(None) => (),
                        Err(e) => return (self.logs.clone(), Err(e)),
                    }
                }
            }
        }
        if let Some(fallback) = self.fallback.as_ref() {
            match fallback.call(&request, method, request.path(), allocator) {
                Ok(Some(response)) =>
                    return (self.logs.clone(), Ok(response)),
                Ok(None) => (),
                Err(e) =>
                    return (self.logs.clone(), Err(e)),
            }
        }
        (self.logs.clone(),
         Ok(errorpage_from_status(HttpResponseStatusCode::NotFound404).into()))
    }
}

/// Routes for all hosts (domains)
pub struct HostsRouter {
    /// Hostnames are stored in lowercased form.
    pub routers: HashMap<KString, Arc<HostRouter>>,
    /// Fallback when either no `Host` header was sent, or it was not
    /// found in `routers`.
    pub fallback: Option<Arc<HostRouter>>,
    /// Logs when there is no fallback handler.
    pub logs: Arc<Mutex<Logs>>,
}

impl HostsRouter {
    pub fn new(fallback: Option<Arc<HostRouter>>,
               logs: Arc<Mutex<Logs>>
    ) -> HostsRouter {
        HostsRouter {
            routers: Default::default(),
            fallback,
            logs
        }
    }

    pub fn add(&mut self,
               hostname: &str,
               hostrouter: Arc<HostRouter>
    ) -> &mut Self {
        if let Some(_old) =
            self.routers.insert(KString::from_string(hostname.to_lowercase()),
                                hostrouter)
        {
            // XX what am I doing in UniqueRouter? What's the best approach?
            warn!("duplicate entry for hostname {hostname:?}, old one dropped");
        }
        self
    }
}
