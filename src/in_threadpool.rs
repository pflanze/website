use std::panic;
use std::sync::Arc;
use std::sync::mpsc::channel;

use anyhow::Result;
use scoped_thread_pool::Pool;

/// Execute function inside thread pool and return its result. Why is
/// this not part of the `threadpool` crate?
pub fn in_threadpool<F, R>(threadpool: Arc<Pool>, f: F) -> Result<R>
where F: FnOnce() -> R + Send,
      R: Send
{
    let (tx, rx) = channel();
    threadpool.scoped(move |scope| {
        scope.execute(move || {
            // Copy of note from Rouille (why is it the case that it can be ignored?):
            // Note that we always resume unwinding afterwards.
            // We can ignore the small panic-safety mechanism of `catch_unwind`.
            let result = panic::catch_unwind(panic::AssertUnwindSafe(f));
            tx.send(result).expect("channel is there and working");
        });
        let msg = rx.recv()?;
        Ok(msg.expect("XXX size business"))
    })
}
