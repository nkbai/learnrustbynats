//! # async-wg
//!
//! Async version WaitGroup for RUST.
//!
//! ## Examples
//!
//! ```rust
//! #[tokio::main]
//! async fn main() {
//!     use async_wg::WaitGroup;
//!
//!     // Create a new wait group.
//!     let wg = WaitGroup::new();
//!
//!     for _ in 0..10 {
//!         let mut wg = wg.clone();
//!         // Add count n.
//!         wg.add(1).await;
//!
//!         tokio::spawn(async move {
//!             // Do some work.
//!
//!             // Done count 1.
//!             wg.done().await;
//!         });
//!     }
//!
//!     // Wait for done count is equal to add count.
//!     wg.await;
//! }
//! ```
//!
//! ## Benchmarks
//!
//! Simple benchmark comparison run on github actions.
//!
//! Code: [benchs/main.rs](https://github.com/jmjoy/async-wg/blob/master/benches/main.rs)
//!
//! ```text
//! test bench_join_handle ... bench:      34,485 ns/iter (+/- 18,969)
//! test bench_wait_group  ... bench:      36,916 ns/iter (+/- 7,555)
//! ```
//!
//! ## License
//!
//! The Unlicense.
//!

use futures_util::lock::Mutex;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tokio::sync::mpsc;

#[derive(Clone)]
/// Enables multiple tasks to synchronize the beginning or end of some computation.
pub struct WaitGroup {
    name: String, //for test
    count: usize,
    tx: mpsc::Sender<()>,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl WaitGroup {
    /// Creates a new wait group and returns the single reference to it.
    ///
    /// # Examples
    /// ```rust
    /// use async_wg::WaitGroup;
    /// let wg = WaitGroup::new();
    /// ```
    pub fn new(name: String, count: usize) -> WaitGroup {
        let mut count2 = count;
        if count2 == 0 {
            count2 = 1;
        }
        let (tx, rx) = mpsc::channel(count2);
        WaitGroup {
            name,
            tx,
            rx: Arc::new(Mutex::new(rx)),
            count,
        }
    }

    /// Done count 1.
    pub async fn done(&mut self) {
        if let Err(e) = self.tx.send(()).await {
            panic!("{} send error", self.name);
        }
    }
    pub async fn wait(&mut self) {
        let mut rx = self.rx.lock().await;
        for i in 0..self.count {
            rx.recv().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::main]
    #[test]
    async fn can_quit() {
        let mut wg = WaitGroup::new("test".into(), 4);
        assert_eq!(wg.count, 4);
        let mut wg2 = wg.clone();
        tokio::spawn(async move {
            tokio::time::delay_for(tokio::time::Duration::from_millis(10)).await;
            wg2.done().await;
            wg2.done().await;
            wg2.done().await;
            wg2.done().await;
        });
        wg.wait().await
    }
}
