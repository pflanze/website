use std::thread::JoinHandle;
use std::{sync::Arc, thread};

use blake3::Hasher;
use kstring::KString;
use rouille::Server;
use scoped_thread_pool::Pool;

use crate::ahtml::AllocatorPool;
use crate::hostrouter::HostsRouter;
use crate::language::Language;
use crate::ppath::PPath;
use crate::webparts::server_handler;


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
