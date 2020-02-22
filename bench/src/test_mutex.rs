use futures_util::lock::Mutex;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

#[derive(Debug, Clone)]
struct A {
    a: Arc<Mutex<i32>>,
}
impl Future for A {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut a = self.a.lock();
        let a = Pin::new(&mut a).poll(cx);
        return match a {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(n) => {
                println!("A poll ={}", *n);
                std::task::Poll::Ready(())
            }
        };
    }
}

pub async fn test_mutex() {
    let a = A {
        a: Arc::new(Mutex::new(3)),
    };
    let a2 = a.clone();
    println!("call spawn");
    tokio::spawn(async move {
        println!("a2 start");
        let a2 = a2.a.lock().await;
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("a2={}", *a2);
    });

    std::thread::sleep(std::time::Duration::from_millis(10));
    println!("a start await");
    a.await;
    println!("a await complete");
}
