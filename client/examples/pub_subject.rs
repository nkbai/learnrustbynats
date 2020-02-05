use client::client::Client;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:4222";
    let mut c = Client::connect(addr).await?;
    for i in 0..10 {
        println!("pub {}", i);
        c.pub_message("test", format!("hello{}", i).as_bytes())
            .await?;
    }
    println!("close connection");
    c.stop.send(());
    Ok(())
}
