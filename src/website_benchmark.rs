use lazy_static::lazy_static;
use std::fmt::Write;
use std::sync::Mutex;
use anyhow::Result;

use crate::arequest::ARequest;
use crate::aresponse::AResponse;
use crate::http_request_method::HttpRequestMethodSimple;
use crate::http_response_status_codes::HttpResponseStatusCode;
use crate::ahtml::{HtmlAllocator, Node};
use crate::webutils::htmlresponse;


struct State {
    counter: i64,
}

lazy_static! {
    static ref STATE: Mutex<State> = Mutex::new(State { counter: 0 });
}

pub fn benchmark<'a>(_request: &ARequest,
                     _method: HttpRequestMethodSimple,
                     alloc: &HtmlAllocator)
            -> Result<AResponse>
{
    htmlresponse(alloc, HttpResponseStatusCode::OK200, |h| {
        let lit = |s| h.staticstr(s);
        let string = |s| h.string(s);
        // let cap = |t| error_boundary(h, t);
        
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
                        // cap(||{
                        //     h.table(
                        //         [],
                        //         [h.table([], [])?])
                        // }),
                        h.table(
                            [],
                            table_body)?,
                    ])?,
            ])
    }).map(AResponse::from)
}

    
