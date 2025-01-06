use std::fmt::Write;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Result, Error};
use lazy_static::lazy_static;
use rouille::Request;
use rouille::Response;
use rouille::router;
use rouille::start_server;

use ahtml::{HtmlAllocatorPool, HtmlAllocator, Node};

use website::http_response_status_codes::HttpResponseStatusCode;
use website::webutils::errorpage_from_error;
use website::webutils::{htmlresponse, errorpage_from_status, error_boundary};

struct State {
    counter: i64,
}

lazy_static! {
    static ref STATE: Mutex<State> = Mutex::new(State { counter: 0 });
}

fn root(alloc: &HtmlAllocator) -> Result<Response> {
    htmlresponse(alloc, HttpResponseStatusCode::OK200, |h| {
        let lit = |s| h.staticstr(s);
        let string = |s| h.string(s);
        let cap = |t| error_boundary(h, t);
        
        let counter: i64 = {
            let mut m = STATE.lock().expect("die too if poisoned");
            m.counter += 1;
            m.counter
        };

        let mut table_body = h.new_vec::<Node>();
        let mut td0 = String::new();
        let mut td1 = String::new();

        td0.clear();
        td0.write_fmt(format_args!("{}abc", counter))?;

        for i in 100..25000 {
            td1.clear();
            td1.write_fmt(format_args!("{} - {}",
                                       i + counter,
                                       if (i as f64 * 0.1).sin() > 0.432 {
                                           "on"
                                       } else {
                                           "off"
                                       }))?;
            table_body.push(h.tr(
                [],
                [
                    h.td(
                        [],
                        [
                            h.str(&td0)?
                        ])?,
                    h.td(
                        [],
                        [
                            h.str(&td1)?,
                        ])?,
                ])?)?;
        }

        h.html(
            [],
            [
                h.head([], [
                    h.title([], [
                        lit("Test page")?,
                        ])?,
                ])?,
                h.body(
                    [
                        // att("bgcolor", "#f0e040")
                    ],
                    [
                        h.p(
                            [],
                            [
                                lit("Hello world!")?,
                            ])?,
                        h.p(
                            [],
                            [
                                lit("Counter: ")?,
                                string(counter.to_string())?,
                            ])?,
                        cap(||{
                            h.table(
                                [],
                                [h.table([], [])?])
                        }),
                        h.table(
                            [],
                            table_body)?,
                    ])?,
            ])
    })
}

lazy_static!{
    static ref ALLOCPOOL: HtmlAllocatorPool =
        // XX config
        HtmlAllocatorPool::new(1000000, true,
                               Arc::new(format!("from {}:{}", file!(), line!())));
}


fn main() -> Result<()> {
    start_server(
        "127.0.0.1:3000",
        move |request: &Request| {
            let clientaddr = request.remote_addr();
            let url = request.url();
            let hds= request.headers();
            let host = request.header("host");
            let method = request.method();
            // sigh split pls ?.
            println!("{clientaddr:?}: {method:?} {host:?} / {url:?} ({hds:?})");
            router!(
                request,
                (GET) (/) => {
                    let mut guard = ALLOCPOOL.get();
                    root(guard.allocator()).or_else(
                        |e| Ok::<Response, Error>(errorpage_from_error(e)))
                        .expect("always OK")
                },
                _ => {
                    errorpage_from_status(HttpResponseStatusCode::NotFound404)
                }
            )
        });
}

