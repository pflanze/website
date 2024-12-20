//! Route from a domain (host name) to a handler of paths (which then
//! routes to a handler for the path).

use std::{sync::{Arc, Mutex}, collections::HashMap};

use kstring::KString;

use ahtml::HtmlAllocator;
use chj_util::warn;

use crate::{router::MultiRouter,
            handler::Handler,
            apachelog::Logs,
            acontext::AContext,
            webutils::errorpage_from_status,
            http_response_status_codes::HttpResponseStatusCode,
            http_request_method::HttpRequestMethodSimple,
            aresponse::AResponse,
            language::Language};

/// Route for a particular host (domain)
pub struct HostRouter<L: Language> {
    pub router: Option<Arc<MultiRouter<Arc<dyn Handler<L>>>>>,
    /// Fallback when no router accepted the path.
    pub fallback: Option<Arc<dyn Handler<L>>>,
    /// Logs when using either routed or fallback handler.
    pub logs: Arc<Mutex<Logs>>,
}

impl<L: Language> HostRouter<L> {
    pub fn handle_request(
        &self,
        context: &AContext<L>,
        method: HttpRequestMethodSimple,
        allocator: &HtmlAllocator
    ) -> (Arc<Mutex<Logs>>, anyhow::Result<AResponse>)
    {
        if let Some(router) = &self.router {
            if let Some((handlers, rest)) = router.get(context.path()) {
                // dt!("multirouter", rest);
                for handler in handlers {
                    match handler.call(&context, method, &rest, allocator) {
                        Ok(Some(response)) => return (self.logs.clone(), Ok(response)),
                        Ok(None) => (),
                        Err(e) => return (self.logs.clone(), Err(e)),
                    }
                }
            }
        }
        if let Some(fallback) = self.fallback.as_ref() {
            match fallback.call(&context, method, context.path(), allocator) {
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
pub struct HostsRouter<L: Language> {
    /// Hostnames are stored in lowercased form.
    pub routers: HashMap<KString, Arc<HostRouter<L>>>,
    /// Fallback when either no `Host` header was sent, or it was not
    /// found in `routers`.
    pub fallback: Option<Arc<HostRouter<L>>>,
    /// Logs when there is no fallback handler.
    pub logs: Arc<Mutex<Logs>>,
}

impl<L: Language> HostsRouter<L> {
    pub fn new(fallback: Option<Arc<HostRouter<L>>>,
               logs: Arc<Mutex<Logs>>
    ) -> Self {
        Self {
            routers: Default::default(),
            fallback,
            logs
        }
    }

    pub fn add(&mut self,
               hostname: &str,
               hostrouter: Arc<HostRouter<L>>
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
