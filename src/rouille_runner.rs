use std::sync::Mutex;
use std::thread::JoinHandle;
use std::{sync::Arc, thread};

use blake3::Hasher;
use kstring::KString;
use rouille::session::session;
use rouille::{Server, Request, Response};
use scoped_thread_pool::Pool;

use crate::acontext::AContext;
use crate::ahtml::AllocatorPool;
use crate::apachelog::{log_combined, Logs};
use crate::aresponse::AResponse;
use crate::hostrouter::HostsRouter;
use crate::http_request_method::HttpRequestMethodGrouped;
use crate::http_response_status_codes::HttpResponseStatusCode;
use crate::in_threadpool::in_threadpool;
use crate::language::Language;
use crate::ppath::PPath;
use crate::webutils::errorpage_from_status;
use crate::{time_guard, warn, time_util};


/// Make a handler for Rouille's `start_server` procedure.
pub fn server_handler<'t, L: Language + Default>(
    listen_addr: String,
    hostsrouter: Arc<HostsRouter<L>>,
    allocatorpool: &'static AllocatorPool,
    threadpool: Arc<Pool>,
    sessionid_hasher: Hasher,
    lang_from_path: Arc<dyn Fn(&PPath<KString>) -> Option<L> + Send + Sync>,
) -> impl for<'r> Fn(&'r Request) -> Response
{
    move |request: &Request| -> Response {
        time_guard!("server_handler"); // timings including infrastructure cost
        let lang_from_path = lang_from_path.clone();
        session(request, "sid", 3600 /*sec*/, |session| {
            let aresponse = in_threadpool(threadpool.clone(), || -> AResponse {
                let okhandler = |context: &AContext<L>| -> AResponse {
                    log_combined(
                        context,
                        || -> (Arc<Mutex<Logs>>, anyhow::Result<AResponse>) {
                            let method = context.method();
                            let unimplemented = |methodname| {
                                warn!("method {methodname:?} not implemented (yet)");
                                (hostsrouter.logs.clone(),
                                 Ok(errorpage_from_status(
                                     HttpResponseStatusCode::NotImplemented501).into()))
                            };
                            match method.to_grouped() {
                                HttpRequestMethodGrouped::Simple(simplemethod) => {
                                    let mut guard = allocatorpool.get();
                                    let allocator = guard.allocator();
                                    if let Some(host) = context.host() {
                                        let lchost = host.to_lowercase();
                                        if let Some(hostrouter) = hostsrouter.routers.get(
                                            &KString::from_string(lchost))
                                        {
                                            return hostrouter.handle_request(
                                                context, simplemethod, allocator)
                                        }
                                    }
                                    if let Some(fallback) = &hostsrouter.fallback {
                                        return fallback.handle_request(
                                            context, simplemethod, allocator)
                                    }
                                }
                                HttpRequestMethodGrouped::Document(documentmethod) => {
                                    return unimplemented(
                                        documentmethod.to_http_request_method().as_str())
                                }
                                HttpRequestMethodGrouped::Special(specialmethod) =>
                                    // XX should at least implement OPTIONS, or ?
                                    return unimplemented(
                                        specialmethod.to_http_request_method().as_str())
                                    // match specialmethod {
                                    //     HttpRequestMethodSpecial::OPTIONS =>
                                    //         return unimplemented(),
                                    //     HttpRequestMethodSpecial::TRACE =>
                                    //         return unimplemented(),
                                    //     HttpRequestMethodSpecial::CONNECT =>
                                    //         return unimplemented(),
                                    // },
                            }
                            (hostsrouter.logs.clone(),
                             Ok(errorpage_from_status(HttpResponseStatusCode::NotFound404)
                                .into()))
                        })
                };
                match AContext::new(request, &listen_addr, session, &sessionid_hasher,
                                    lang_from_path) {
                    Ok(context) => {
                        let mut aresponse= okhandler(&context);
                        context.set_headers(&mut aresponse.response.headers);
                        aresponse
                    }
                    Err(e) => {
                        warn!("{e}");
                        errorpage_from_status(
                            HttpResponseStatusCode::InternalServerError500).into()
                    }
                }
            }).expect("only ever fails if thread fails outside catch_unwind");
            let AResponse { response, sleep_until } = aresponse;
            if let Some(t) = sleep_until {
                time_util::sleep_until(t);
            }
            response
        })
    }
}


pub struct Tlskeys {
    pub crt: Vec<u8>,
    pub key: Vec<u8>,
}

pub struct RouilleRunner<L: Language> {
    workerthreadpool: Arc<Pool>,
    allocpool: &'static AllocatorPool,
    sessionid_hasher: Hasher,
    lang_from_path: Arc<dyn Fn(&PPath<KString>) -> Option<L> + Send + Sync>,
}

impl<L: Language + 'static> RouilleRunner<L> {
    pub fn new(
        allocpool: &'static AllocatorPool,
        sessionid_hasher: Hasher,
        lang_from_path: Arc<dyn Fn(&PPath<KString>) -> Option<L> + Send + Sync>,
    ) -> Self
    {
        // The worker thread pool is kept separate and much smaller, since
        // it keeps thread local state, also want CPU intensive part to
        // finish quickly.
        let workerthreadpool_size = 8 * thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let workerthreadpool = {
            let cfg = scoped_thread_pool::ThreadConfig::new()
                .prefix("scoped_website_worker");
            Arc::new(scoped_thread_pool::Pool::with_thread_config(
                workerthreadpool_size, cfg))
        };
        RouilleRunner {
            workerthreadpool,
            allocpool,
            sessionid_hasher,
            lang_from_path,
        }
    }

    /// Run a rouille server in a new thread, and using the shared
    /// worker thread pool.
    pub fn run_server(
        &self,
        thread_name: &str,
        addr: String,
        tlskeys: Option<Tlskeys>,
        hostsrouter: Arc<HostsRouter<L>>,
    ) -> Result<JoinHandle<()>, std::io::Error>
    {
        thread::Builder::new().name(thread_name.into()).spawn({
            let workerthreadpool = self.workerthreadpool.clone();
            let sessionid_hasher = self.sessionid_hasher.clone();
            let lang_from_path = self.lang_from_path.clone();
            let allocpool = self.allocpool;
            move || {
                let handler = server_handler(
                    addr.clone(),
                    hostsrouter,
                    allocpool,
                    workerthreadpool,
                    sessionid_hasher,
                    lang_from_path,
                );
                if let Some(Tlskeys { crt, key }) = tlskeys {
                    Server::new_ssl(addr, handler, crt, key)
                } else {
                    Server::new(addr, handler)
                }
                // Panicking instead returning Result for size issues,
                // and it's run in a dedicated thread where panicking
                // will achieve the same outcome. Bad for WASM or
                // embedded contexts though. TODO fix.
                .expect("error starting server")
                    .run()
            }
        })
    }
}
