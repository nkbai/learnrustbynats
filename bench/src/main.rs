mod bench;
mod wait_group;
use crate::bench::{msgs_per_client, Benchmark, Sample};
use client::client::Client;
use std::error::Error;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::sync::{oneshot, Mutex};
use tokio::time::Instant;
use wait_group::WaitGroup;

/// benchmark for simple nats
#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "simple nats")]
struct Opt {
    ///The nats server URLs (separated by comma)
    #[structopt(long, default_value = "127.0.0.1:4222")]
    urls: String,
    ///Save bench data to csv file
    #[structopt(long, default_value = "")]
    csv_file: String,
    ///Number of Concurrent Publishers
    #[structopt(long, default_value = "1")]
    num_pubs: usize,
    ///Number of Concurrent Subscribers
    #[structopt(long, default_value = "0")]
    num_subs: usize,
    ///Number of Messages to Publish
    #[structopt(long, default_value = "100000")]
    num_msgs: usize,
    ///Size of the message.
    #[structopt(long, default_value = "128")]
    msg_size: usize,
    ///publish subject
    #[structopt(long, default_value = "test_subject")]
    subject: String,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt: Opt = Opt::from_args();
    println!("opt={:?}", opt);
    println!("Hello, world!");
    let start_wg = WaitGroup::new();
    let done_wg = WaitGroup::new();
    let bench = Arc::new(Mutex::new(Benchmark::new("Nats")));
    done_wg.add((opt.num_pubs + opt.num_subs) as isize).await;

    start_wg.add(opt.num_subs as isize).await;

    for _ in 0..opt.num_subs {
        let mut c = Client::connect(opt.urls.as_str()).await.unwrap();
        let start_wg = start_wg.clone();
        let done_wg = done_wg.clone();
        let bench = bench.clone();
        let opt = opt.clone();
        tokio::spawn(async move {
            run_subscriber(&mut c, start_wg, done_wg, opt, bench).await;
            c.close();
        });
    }
    start_wg.await;
    println!("subs all started.");
    let start_wg = WaitGroup::new();

    start_wg.add(opt.num_pubs as isize).await;
    let pub_counts = msgs_per_client(opt.num_msgs, opt.num_pubs);
    for i in 0..opt.num_pubs {
        let mut c = Client::connect(opt.urls.as_str()).await.unwrap();
        let start_wg = start_wg.clone();
        let done_wg = done_wg.clone();
        let bench = bench.clone();
        let opt = opt.clone();
        let num_msgs = pub_counts[i];
        tokio::spawn(async move {
            run_publiser(&c, start_wg, done_wg, num_msgs, opt, bench).await;
            c.close();
        });
    }
    start_wg.await;
    println!("pubs all started.");
    done_wg.await;
    println!("all task stopped.");
    println!("{}\n", bench.lock().await.report());
    if opt.csv_file.len() > 0 {
        tokio::fs::write(opt.csv_file.as_str(), bench.lock().await.csv())
            .await
            .unwrap();
        println!("saved metric data in csv file {}", opt.csv_file);
    }
    Ok(())
}

async fn run_publiser(
    c: &Client,
    start_wg: WaitGroup,
    done_wg: WaitGroup,
    num_msgs: usize,
    opt: Opt,
    bench: Arc<Mutex<Benchmark>>,
) {
    start_wg.done().await;
    let msg = vec![0x33; opt.msg_size];
    let start = Instant::now();
    for _ in 0..num_msgs {
        let _ = c.pub_message(opt.subject.as_str(), msg.as_slice()).await;
    }
    let s = Sample::new(
        num_msgs,
        opt.msg_size,
        num_msgs as u64,
        (num_msgs * opt.msg_size) as u64,
        start,
        Instant::now(),
    );
    bench.lock().await.add_pub_sample(s);
    done_wg.done().await;
    println!("one pub stoped.");
}

async fn run_subscriber(
    c: &mut Client,
    start_wg: WaitGroup,
    done_wg: WaitGroup,
    opt: Opt,
    bench: Arc<Mutex<Benchmark>>,
) {
    start_wg.done().await;
    let start = Instant::now();
    let mut received_msgs = 0;
    let mut received_bytes = 0;
    let (tx, rx) = oneshot::channel();
    let mut tx = Some(tx);
    let expected_msgs = opt.num_msgs;
    let _ = c
        .sub_message(
            opt.subject.clone(),
            None,
            Box::new(move |msg| {
                received_msgs += 1;
                received_bytes += msg.len();
                if received_msgs >= expected_msgs {
                    if let Some(tx) = tx.take() {
                        let _ = tx.send((received_msgs, received_bytes));
                        println!("sub end.");
                    }
                }
                Ok(())
            }),
        )
        .await;
    let (received_msgs, received_bytes) = rx.await.unwrap();
    let s = Sample::new(
        opt.num_msgs,
        opt.msg_size,
        received_msgs as u64,
        received_bytes as u64,
        start,
        Instant::now(),
    );
    bench.lock().await.add_sub_sample(s);
    done_wg.done().await;
}
