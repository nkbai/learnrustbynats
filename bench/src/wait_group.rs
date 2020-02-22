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

#[derive(Clone)]
/// Enables multiple tasks to synchronize the beginning or end of some computation.
pub struct WaitGroup {
    inner: Arc<Inner>,
    name: String, //for test
}

struct Inner {
    started: AtomicBool,
    count_and_waker: Mutex<CountAndWaker>,
}
/*
如果count和waker不是同一把锁,会出现如下情况
1. poll 发现 count 没有ready
2. poll 被调度走
3. done count-1,发现到0了
4. done 尝试wake,发现没有必要,因为 poll正在执行.
5. poll被调度回来
6. poll继续,设置awaker
7. 但是没有任何人会awake他了.
*/
struct CountAndWaker {
    count: isize,
    waker: Option<Waker>,
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
        WaitGroup {
            inner: Arc::new(Inner {
                started: AtomicBool::new(false),
                count_and_waker: Mutex::new(CountAndWaker {
                    count: 0,
                    waker: None,
                }),
            }),
            name: name.into(),
        }
    }

    /// Add count n.
    ///
    /// # Panic
    /// 1. The argument `delta` must be a non negative number (> 0).
    /// 2. The max count must be less than `isize::max_value()`.
    pub async fn add(&self, delta: isize) {
        if delta < 0 {
            panic!("The argument `delta` of wait group `add` must be a positive number");
        }
        if self
            .inner
            .started
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            panic!("{} cannot add after started.", self.name);
        }
        let mut count_and_waker = self.inner.count_and_waker.lock().await;
        count_and_waker.count += delta;
        eprintln!(
            "{} add {}, result={}",
            self.name, delta, count_and_waker.count
        );
        if count_and_waker.count >= isize::max_value() {
            panic!("{} wait group count is too large", self.name);
        }
    }

    /// Done count 1.
    pub async fn done(&self) {
        eprintln!("{} call done", self.name);
        let mut count_and_waker = self.inner.count_and_waker.lock().await;
        count_and_waker.count -= 1;
        if count_and_waker.count < 0 {
            panic!("{} done must equal add", self.name);
        }
        eprintln!("{} call done count={}", self.name, count_and_waker.count);
        if count_and_waker.count <= 0 {
            {
                eprintln!("{} all done", self.name);
                if let Some(waker) = count_and_waker.waker.take() {
                    drop(count_and_waker);
                    eprintln!("drop now");
                    eprintln!("{} call wake", self.name);
                    waker.clone().wake();
                    eprintln!("{} wake complete", self.name);
                } else {
                    eprintln!("{} done before any await", self.name);
                }
            }
            eprintln!("{} droped", self.name);
        }
    }

    /// Get the inner count of `WaitGroup`, the primary count is `0`.
    pub async fn count(&self) -> isize {
        self.inner.count_and_waker.lock().await.count
    }
}

impl Future for WaitGroup {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.inner
            .started
            .store(true, std::sync::atomic::Ordering::SeqCst);
        eprintln!("{} wait group polled.", self.name);
        let mut count_and_waker = self.inner.count_and_waker.lock();
        let pin_count = Pin::new(&mut count_and_waker);
        if let Poll::Ready(mut count_and_waker) = pin_count.poll(cx) {
            if count_and_waker.count <= 0 {
                eprintln!("{} ready", self.name);
                return Poll::Ready(());
            }
            /*
            返回pending有两种情况
            1. Mutex返回了Pending,那么有其他人释放锁的时候会唤醒
            2. Mutex返回了Ready,但是发现count>0,那么这时候是我返回Pending,这时候我就需要设置好
            */
            eprintln!("{} wait group set waker", self.name);
            count_and_waker.waker = Some(cx.waker().clone());
        }
        eprintln!("{} wait group not ready", self.name);

        Poll::Pending
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
        wg.await
    }

    #[tokio::test]
    #[should_panic]
    async fn add_neg_one() {
        let wg = WaitGroup::new("aa");
        wg.add(-1).await;
    }

    #[tokio::main]
    #[test]
    #[should_panic]
    async fn add_very_max() {
        let wg = WaitGroup::new("aa");
        wg.add(isize::max_value()).await;
    }

    #[tokio::main]
    #[test]
    async fn add() {
        let wg = WaitGroup::new("aa");
        wg.add(1).await;
        wg.add(10).await;
        assert_eq!(wg.count().await, 11);
    }

    #[tokio::main]
    #[test]
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
    #[tokio::main]
    #[test]
    async fn can_quit() {
        let wg = WaitGroup::new("aa");
        assert_eq!(wg.count().await, 0);
        wg.add(1).await;
        let wg2 = wg.clone();
        tokio::spawn(async move {
            tokio::time::delay_for(tokio::time::Duration::from_millis(10)).await;
            wg2.done().await;
        });
        wg.await
    }
}
