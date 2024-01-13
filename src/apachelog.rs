//! Write HTTP access log files in the Combined Log Format (extended
//! Common Log Format) for access logs (Apache style), as per
//! <https://httpd.apache.org/docs/2.4/logs.html>.

use std::mem::swap;
use std::panic;
use std::sync::{Arc, Mutex};
use std::{time::{Duration, SystemTime, Instant}, io::{stderr, BufWriter}};
use std::io::Write;

use anyhow::Result;
use chrono::{DateTime, Utc, Datelike, Timelike};
use rouille::ResponseBody;

use crate::arequest::ARequest;
use crate::aresponse::AResponse;
use crate::easy_fs::open_log_output;
use crate::http_response_status_codes::HttpResponseStatusCode;
use crate::try_result;
use crate::webutils::errorpage_from_status;

// How many times will I write these up?
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

// "06/Dec/2023:02:02:47 +0100"
pub fn write_time(
    outp: &mut impl Write,
    time: SystemTime
) -> Result<()> {
    let dt: DateTime<Utc> = DateTime::from(time);
    write!(outp, "{:02}/{}/{:04}:{:02}:{:02}:{:02} +0000",
           dt.day(), MONTHS[dt.month0() as usize], dt.year(),
           dt.hour(), dt.minute(), dt.second())?;
    Ok(())
}

// Apache:
// 18.134.151.89 - - [06/Dec/2023:02:02:47 +0100] "GET /login.jsp HTTP/1.1" 404 447 "-" "'Cloud mapping experiment. Contact research@pdrlabs.net'"
// 44.212.94.18 - - [06/Dec/2023:02:38:18 +0100] "GET /resume/nontechnical.html HTTP/1.1" 200 2403 "-" "CCBot/2.0 (https://commoncrawl.org/faq/)"
// 194.38.22.71 - - [06/Dec/2023:03:08:43 +0100] "GET /public/assets/plugins/plupload/examples/upload.php HTTP/1.1" 404 572 "-" "ALittle Client"
// We also add duration at the end.

/// Write to access.log; Not sure yet about how to handle Error XX
pub fn write_combined(
    outp: &mut impl Write,
    request: &ARequest,
    duration: Duration,
    aresponse: &mut AResponse, // temporarily swaps out ResponseBody and back
) -> Result<()> {
    // Write the time when the log entry is made, not when the
    // request started
    let now = SystemTime::now();
    write!(outp, "{} - - [", request.client_ip())?;
    write_time(outp, now)?;
    let len = {
        // Total HACK to get at the response body length, since those
        // fields are private and there are no accessors, we have to
        // become drastic:
        let mut responsebody = ResponseBody::empty();
        swap(&mut responsebody, &mut aresponse.response.data);
        let (data, length) = responsebody.into_reader_and_size();
        let len = length.clone();
        responsebody =
            if let Some(len) = length {
                ResponseBody::from_reader_and_size(data, len)
            } else {
                ResponseBody::from_reader(data)
            };
        swap(&mut responsebody, &mut aresponse.response.data);
        len
    };
    writeln!(outp, "] {:?} {} {} {:?} {:?} {duration:?}",
             request.request_line(),
             aresponse.response.status_code,
             len.unwrap_or(0), // XX hack, is missing headers and compression and missing at all
             request.referer().unwrap_or("-"),
             request.user_agent().unwrap_or("-") // XX or what as alternative?
    )?;
    outp.flush()?;
    Ok(())
}


// Apache:
// [Wed Dec 06 03:40:39 2023] [error] [client 45.95.147.204] File does not exist: /var/www/default/boaform, referer: http://159.100.250.224:80/admin/login.asp
// [Wed Dec 06 03:44:41 2023] [error] [client 142.132.237.69] File does not exist: /var/www/christianjaeger.ch/debs
// But we don't need to follow this.

/// Write to error.log
fn write_error(
    outp: &mut impl Write,
    request: &ARequest,
    duration: Duration,
    err: anyhow::Error,
) -> Result<()> {
    let now = SystemTime::now();
    write!(outp, "[")?;
    write_time(outp, now)?;
    writeln!(outp, "] [error] [client {}] {:?} {duration:?}: {err:#}",
             request.client_ip(),
             request.request_line())?;
    outp.flush()?;
    Ok(())
}

/// Panic log to stderr. Panics on errors logging to stderr.
fn write_panic_stderr(
    request: &ARequest,
    duration: Duration
) {
    try_result!{
        let mut outp = BufWriter::new(stderr().lock());
        // let now = SystemTime::now();
        // write_time(&mut outp, now)?;
        // We need to feed stderr to a service like daemontools
        // anyway, hence don't print timestamps.
        writeln!(&mut outp, "[panic] handling {:?} after {duration:?}",
                 request.request_line())?;
        outp.flush()?;
        Ok(())
    }.expect("stderr always writable");
}


// Can't actually make use of rouille::log_custom for the logging:

// * we don't have the site or log file to write to, except via a
//   closure (or thread-local variable, not wanting to go there)
// * we don't have the possible Error result except via a closure (or
//   thread-local variable)
// * we only get those things within the handler, but at that point
//   the closure needed to be passed already.
// * we can't sensibly pass a fake handler since that would be outside
//   the scope of the panic handler that's within log_custom.

// Thus instead, copy and adapt its code.


/// The log files to write to, either access_log if successful, or
/// error log when no response (even templated one) was made (XX
/// hmm). Should do buffering (i.e. be BufWriter), the code calls
/// flush once per entry.
pub struct Logs {
    pub access_log: Box<dyn Write + Send + Sync>,
    pub error_log: Box<dyn Write + Send + Sync>,
}

impl Logs {
    pub fn open_in_basedir(
        logbasedir: &str,
        is_https: bool
    ) -> Result<Arc<Mutex<Logs>>>
    {
        let s = if is_https { "s" } else { "" };
        Ok(Arc::new(Mutex::new(Logs {
            access_log: open_log_output(
                format!("{logbasedir}/http{s}_access.log"))?,
            error_log: open_log_output(
                format!("{logbasedir}/http{s}_error.log"))?,
        })))
    }
}


pub fn log_combined<F>(
    request: &ARequest,
    handler: F
) -> AResponse
where
    F: FnOnce() -> (Arc<Mutex<Logs>>, anyhow::Result<AResponse>),
{
    let start_instant = Instant::now();

    // Call the handler and catch panics.
    // Note that we always resume unwinding afterwards.
    // We can ignore the small panic-safety mechanism of `catch_unwind`. -- Why?
    let result = panic::catch_unwind(panic::AssertUnwindSafe(handler));
    let elapsed = start_instant.elapsed();

    match result {
        Ok((logs, result)) => match result {
            Ok(mut response) => {
                {
                    let mut _logs = logs.lock().expect(
                        "if `write` panics then we are lost anyway");
                    match write_combined(&mut _logs.access_log, request, elapsed, &mut response)
                    {
                        Ok(()) => (),
                        Err(e) => eprintln!("WARNING: could not write to access log: {}", e)
                    }
                }
                response
            }
            Err(err) => {
                {
                    let mut _logs = logs.lock().expect(
                        "if `write` panics then we are lost anyway");
                    match write_error(&mut _logs.error_log, request, elapsed, err) {
                        Ok(()) => (),
                        Err(e) => eprintln!("WARNING: could not write to access log: {}", e)
                    }
                }
                // XX btw expects that the requester accepts HTML. Not always OK?
                errorpage_from_status(HttpResponseStatusCode::InternalServerError500)
                    .into()
            }
        },
        Err(payload) => {
            write_panic_stderr(request, elapsed);
            // The panic handler will print the payload contents
            panic::resume_unwind(payload);
        }
    }
}

