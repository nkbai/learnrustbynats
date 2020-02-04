use crate::error::*;
use crate::parser::{ParseResult, Parser, PubArg, SubArg};
use crate::server::*;
use crate::simple_sublist::{ArcSubscription, SubListTrait, Subscription};
use rand::{RngCore, SeedableRng};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::io::{BufWriter, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Client<T: SubListTrait> {
    pub srv: Arc<Mutex<ServerState<T>>>,
    pub cid: u64,
    pub msg_sender: Arc<Mutex<ClientMessageSender>>,
}
#[derive(Debug)]
pub struct ClientMessageSender {
    writer: WriteHalf<TcpStream>,
}
impl ClientMessageSender {
    pub fn new(writer: WriteHalf<TcpStream>) -> Self {
        Self { writer }
    }
}
impl<T: SubListTrait + Send + 'static> Client<T> {
    pub fn process_connection(
        cid: u64,
        srv: Arc<Mutex<ServerState<T>>>,
        conn: TcpStream,
    ) -> Arc<Mutex<ClientMessageSender>> {
        println!("process new connection id={}", cid);
        let (mut reader, writer) = tokio::io::split(conn);
        let msg_sender = Arc::new(Mutex::new(ClientMessageSender::new(writer)));
        let c = Client {
            srv,
            cid,
            msg_sender: msg_sender.clone(),
        };
        tokio::spawn(async move {
            c.client_task(reader).await;
        });
        return msg_sender;
    }
    async fn client_task(self, mut reader: ReadHalf<TcpStream>) {
        use futures::StreamExt;
        use tokio::io::{AsyncRead, AsyncReadExt};
        let mut subs = HashMap::new();
        let mut parser = Parser::new();
        let mut buf = [0; 1024];
        loop {
            let n = reader.read(&mut buf).await;
            if n.is_err() {
                self.process_error(Box::new(n.unwrap_err()), subs).await;
                return;
            }
            let n = n.unwrap();
            let mut buf = &buf[0..n];
            loop {
                let r = parser.parse(&buf[..]);
                println!("client {} receive message {:?}", self.cid, r);
                if r.is_err() {
                    self.process_error(Box::new(r.unwrap_err()), subs).await;
                    return;
                }
                use crate::parser::ParseResult::*;
                let (r, n) = r.unwrap();
                match r {
                    NoMsg => {
                        break; //没有读取到完整的消息,继续下一条
                    }
                    Pub(pub_arg) => {
                        let r = self.process_pub(&pub_arg).await;
                        if r.is_err() {
                            self.process_error(Box::new(r.unwrap_err()), subs).await;
                            return;
                        }
                        parser.clear_msg_buf();
                    }
                    Sub(sub) => {
                        let r = self.process_sub(&sub, &mut subs).await;
                        if r.is_err() {
                            self.process_error(r.unwrap_err(), subs).await;
                            return;
                        }
                    }
                }
                if n == buf.len() {
                    break;
                }
                buf = &buf[n..];
            }
        }
    }
    async fn process_error<E: Error>(&self, err: E, subs: HashMap<String, ArcSubscription>) {
        println!("client {} process err {:?}", self.cid, err);
        {
            let mut srv = self.srv.lock().await;
            for s in subs {
                if let Err(e) = srv.sublist.remove(s.1.clone()) {
                    println!(
                        "client {} remove sub {} error {:?}",
                        self.cid, &s.1.subject, e
                    );
                }
            }
            drop(srv); //提前释放
        }
        let mut writer = self.msg_sender.lock().await;
        writer.writer.shutdown().await;
    }
    async fn process_sub(
        &self,
        sub: &SubArg<'_>,
        subs: &mut HashMap<String, ArcSubscription>,
    ) -> Result<()> {
        let s = Subscription {
            msg_sender: self.msg_sender.clone(),
            subject: sub.subject.to_string(),
            queue: sub.queue.map(|s| s.to_string()),
            sid: sub.sid.to_string(),
        };
        let s = Arc::new(s);
        subs.insert(sub.subject.to_string(), s.clone());
        let mut srv = self.srv.lock().await;
        srv.sublist.insert(s)
    }
    async fn process_pub(&self, pub_arg: &PubArg<'_>) -> Result<()> {
        let subject = pub_arg.subject;
        let s = self.srv.lock().await.sublist.match_subject(&subject)?;
        for sub in s.subs.iter() {
            self.send_message(sub.as_ref(), &pub_arg)
                .await
                .map_err(|e| {
                    println!("send message to client err {}", e);
                    NError::new(ERROR_UNKOWN_ERROR)
                })?;
        }
        let mut r = rand::rngs::StdRng::from_entropy();
        for qsub in s.qsubs.iter() {
            println!("subject={},qsubs len={}", subject, qsub.len());
            let pos = r.next_u32() as usize % qsub.len();
            let sub = qsub.get(pos).expect("qsub must have at least one element");
            self.send_message(sub.as_ref(), &pub_arg)
                .await
                .map_err(|_| NError::new(ERROR_UNKOWN_ERROR));
        }
        Ok(())
    }
    async fn send_message(&self, sub: &Subscription, pub_arg: &PubArg<'_>) -> std::io::Result<()> {
        let mut writer = sub.msg_sender.lock().await;
        use tokio::io::BufWriter;
        writer.writer.write("MSG ".as_bytes()).await?;
        writer.writer.write(sub.subject.as_bytes()).await?;
        writer.writer.write(" ".as_bytes()).await?;
        writer.writer.write(sub.sid.as_bytes()).await?;
        writer.writer.write(" ".as_bytes()).await?;
        writer.writer.write(pub_arg.size_buf.as_bytes()).await?;
        writer.writer.write("\r\n".as_bytes()).await?;
        writer.writer.write(pub_arg.msg).await?;
        writer.writer.write("\r\n".as_bytes()).await?;
        Ok(())
    }
}

pub fn new_test_tcp_writer() -> Arc<Mutex<ClientMessageSender>> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let conn = std::net::TcpStream::connect(l.local_addr().unwrap()).unwrap();
    let conn = tokio::net::TcpStream::from_std(conn).unwrap();
    let (_, writer) = tokio::io::split(conn);
    return Arc::new(Mutex::new(ClientMessageSender::new(writer)));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_rng() {
        for _ in 0..10 {
            let mut r = rand::rngs::StdRng::from_entropy();
            println!("next={}", r.next_u32());
        }
    }
}
