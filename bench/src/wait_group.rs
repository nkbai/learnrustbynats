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
//!     wg.wait().await;
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

use futures::future::ok;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicIsize;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tokio::sync::oneshot;
use tokio::sync::Mutex;

#[derive(Clone)]
/// Enables multiple tasks to synchronize the beginning or end of some computation.
pub struct WaitGroup {
    inner: Arc<Inner>,
}

struct Inner {
    started: AtomicBool,
    count: AtomicIsize,
    sender: Mutex<Option<oneshot::Sender<()>>>,
    receiver: Mutex<Option<oneshot::Receiver<()>>>,
}

impl WaitGroup {
    /// Creates a new wait group and returns the single reference to it.
    ///
    /// # Examples
    /// ```rust
    /// use async_wg::WaitGroup;
    /// let wg = WaitGroup::new();
    /// ```
    pub fn new<T: Into<String>>(name: T) -> WaitGroup {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        WaitGroup {
            inner: Arc::new(Inner {
                started: AtomicBool::new(false),
                sender: Mutex::new(Some(sender)),
                receiver: Mutex::new(Some(receiver)),
                count: AtomicIsize::new(0),
            }),
        }
    }

    /// Add count n.
    ///
    /// # Panic
    /// 1. The argument `delta` must be a non negative number (>= 0).
    pub async fn add(&self, delta: isize) {
        if delta < 0 {
            panic!("The argument `delta` of wait group `add` must be a positive number");
        }
        if self
            .inner
            .started
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            panic!("cannot add after started.");
        }
        self.inner
            .count
            .fetch_add(delta, std::sync::atomic::Ordering::Release);
    }

    /// Done count 1.
    pub async fn done(&self) {
        let count = self
            .inner
            .count
            .fetch_sub(1, std::sync::atomic::Ordering::Acquire);
        // eprintln!("{} call done,count={}", self.name, count);
        if count <= 0 {
            panic!("done add not match");
        }
        if count == 1 {
            let mut sender = self.inner.sender.lock().await.take().unwrap();
            let _ = sender.send(());
        }
    }

    /// Get the inner count of `WaitGroup`, the primary count is `0`.
    pub async fn count(&self) -> isize {
        self.inner.count.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// wait all done, if count() is zero ,returns immediately.
    pub async fn wait(&self) {
        self.inner
            .started
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if self.inner.count.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            return;
        }
        let receiver = self.inner.receiver.lock().await.take().unwrap();
        let _ = receiver.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::main]
    #[test]
    async fn add_zero() {
        let wg = WaitGroup::new("aa");
        wg.add(0).await;
        //        wg.done().await;
        wg.wait().await
    }

    #[tokio::test]
    async fn add() {
        let wg = WaitGroup::new("aa");
        wg.add(1).await;
        wg.add(10).await;
        assert_eq!(wg.count().await, 11);
    }

    #[tokio::test]
    #[should_panic]
    async fn done() {
        let wg = WaitGroup::new("aa");
        wg.done().await;
        wg.done().await; //done次数超过了await
        assert_eq!(wg.count().await, -2);
    }

    #[tokio::main]
    #[test]
    async fn count() {
        let wg = WaitGroup::new("aa");
        assert_eq!(wg.count().await, 0);
        wg.add(10).await;
        assert_eq!(wg.count().await, 10);
        wg.done().await;
        assert_eq!(wg.count().await, 9);
    }
    #[tokio::test]
    async fn can_quit() {
        let wg = WaitGroup::new("aa");
        assert_eq!(wg.count().await, 0);
        wg.add(1).await;
        let wg2 = wg.clone();
        tokio::spawn(async move {
            println!("spawn run");
            tokio::time::delay_for(tokio::time::Duration::from_millis(10)).await;
            wg2.done().await;
        });
        wg.wait().await
    }
}
