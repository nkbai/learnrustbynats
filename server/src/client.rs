use crate::error::*;
use crate::parser::{ParseResult, Parser, PubArg, SubArg};
use crate::server::*;
use crate::simple_sublist::{ArcSubResult, ArcSubscription, SubListTrait, Subscription};
use rand::{RngCore, SeedableRng};
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::sync::Arc;
use tokio::io::*;
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
    writer: Option<WriteHalf<TcpStream>>,
    msg_buf: Option<Vec<u8>>,
}
impl ClientMessageSender {
    pub fn new(writer: WriteHalf<TcpStream>) -> Self {
        Self {
            writer: Some(writer),
            msg_buf: Some(Vec::with_capacity(512)),
        }
    }
    async fn send_all(&mut self) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.writer {
            let r = writer
                .write_all(self.msg_buf.as_ref().unwrap().as_slice())
                .await;
            self.msg_buf.as_mut().unwrap().clear();
            r
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone)]
pub struct ClientMessageSenderWrapper(Arc<Mutex<ClientMessageSender>>, usize);
impl std::cmp::PartialEq for ClientMessageSenderWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
/*
为了能够将ArcSubscription,必须实现下面这些Trait
*/
impl std::cmp::Eq for ClientMessageSenderWrapper {}
impl std::cmp::PartialOrd for ClientMessageSenderWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for ClientMessageSenderWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1)
    }
}
impl<T: SubListTrait + Send + 'static> Client<T> {
    pub fn process_connection(
        cid: u64,
        srv: Arc<Mutex<ServerState<T>>>,
        conn: TcpStream,
    ) -> Arc<Mutex<ClientMessageSender>> {
        let (reader, writer) = tokio::io::split(conn);
        let msg_sender = Arc::new(Mutex::new(ClientMessageSender::new(writer)));
        let c = Client {
            srv: srv,
            cid,
            msg_sender: msg_sender.clone(),
        };
        tokio::spawn(async move {
            Client::client_task(c, reader).await;
            println!("client {}  client_task quit", cid);
        });
        msg_sender
    }
    async fn client_task(self, mut reader: ReadHalf<TcpStream>) {
        let mut parser = Parser::new();
        let mut count: i32 = 0;
        let mut subs = HashMap::new();
        let mut buf = [0; 1024 * 64];
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut cache = HashMap::new();
        let mut pendings = BTreeSet::new();
        loop {
            //            let mut buf: Vec<u8> = Vec::new();
            //            let r = tokio::io::copy(&mut reader, &mut buf).await;
            //            match r {
            //                Ok(r) => println!("recevied {} bytes", r),
            //                Err(e) => println!("copy err: {}", e),
            //            }
            //            return;
            count += 1;
            let r = reader.read(&mut buf[..]).await;
            if r.is_err() {
                let e = r.unwrap_err();
                self.process_error(e, subs).await;
                return;
            }
            let r = r.unwrap();
            let n = r;
            if n == 0 {
                self.process_error(NError::new(ERROR_CONNECTION_CLOSED), subs)
                    .await;
                return;
            }
            //            pendings.clear();
            let mut buf2 = &buf[0..n];
            loop {
                let r = parser.parse(&buf2[..]);
                if r.is_err() {
                    {
                        let s = unsafe { std::str::from_utf8_unchecked(&buf2[..]) };
                        println!("parse err buf={}", s);
                    }
                    self.process_error(r.unwrap_err(), subs).await;
                    return;
                }
                let (result, left) = r.unwrap();

                match result {
                    ParseResult::NoMsg => {
                        break;
                    }
                    ParseResult::Sub(ref sub) => {
                        if let Err(e) = self.process_sub(sub, &mut subs).await {
                            self.process_error(e, subs).await;
                            return;
                        }
                    }
                    ParseResult::Pub(ref pub_arg) => {
                        if let Err(e) = self
                            .process_pub(pub_arg, &mut cache, &mut rng, &mut pendings)
                            .await
                        {
                            self.process_error(e, subs).await;
                            return;
                        }
                        parser.clear_msg_buf();
                    }
                }
                if left == buf2.len() {
                    break;
                }
                buf2 = &buf2[left..];
            }
            //批量处理发送
            for c in pendings.iter() {
                let c = c.clone();
                tokio::spawn(async move {
                    let mut sender = c.0.lock().await;
                    if let Err(e) = sender.send_all().await {
                        println!("send_all error {}", e);
                    }
                });
            }
            pendings.clear();
        }
    }
    async fn process_error<E: Error>(&self, err: E, subs: HashMap<String, ArcSubscription>) {
        println!("client {} process err {:?}", self.cid, err);
        {
            let sublist = &mut self.srv.lock().await.sublist;
            for (_, sub) in subs {
                if let Err(e) = sublist.remove(sub) {
                    println!("client {} remove err {} ", self.cid, e);
                }
            }
        }
        let mut sender = self.msg_sender.lock().await;
        if let Some(mut writer) = sender.writer.take() {
            sender.msg_buf.take();
            if let Err(e) = writer.shutdown().await {
                println!("shutdown err {:?}", e);
            }
        }
    }
    async fn process_sub(
        &self,
        sub: &SubArg<'_>,
        subs: &mut HashMap<String, ArcSubscription>,
    ) -> crate::error::Result<()> {
        let sub = Subscription {
            subject: sub.subject.to_string(),
            queue: sub.queue.map(|q| q.to_string()),
            sid: sub.sid.to_string(),
            msg_sender: self.msg_sender.clone(),
        };
        let sub = Arc::new(sub);
        subs.insert(sub.subject.clone(), sub.clone());
        let sublist = &mut self.srv.lock().await.sublist;
        sublist.insert(sub)?;
        Ok(())
    }
    async fn process_pub(
        &self,
        pub_arg: &PubArg<'_>,
        cache: &mut HashMap<String, ArcSubResult>,
        rng: &mut rand::rngs::StdRng,
        pendings: &mut BTreeSet<ClientMessageSenderWrapper>,
    ) -> crate::error::Result<()> {
        let sub_result = {
            if let Some(r) = cache.get(pub_arg.subject) {
                Arc::clone(r)
            } else {
                let sub_list = &mut self.srv.lock().await.sublist;
                let r = sub_list.match_subject(pub_arg.subject);
                cache.insert(pub_arg.subject.to_string(), Arc::clone(&r));
                r
            }
        };
        if sub_result.psubs.len() > 0 {
            for sub in sub_result.psubs.iter() {
                self.send_message(sub.as_ref(), pub_arg, pendings)
                    .await
                    .map_err(|e| {
                        println!("send message error {}", e);
                        NError::new(ERROR_CONNECTION_CLOSED)
                    })?;
            }
        }
        if sub_result.qsubs.len() > 0 {
            //qsubs 要考虑负载均衡问题
            for qsubs in sub_result.qsubs.iter() {
                let n = rng.next_u32();
                let n = n as usize % qsubs.len();
                let sub = qsubs.get(n).unwrap();
                self.send_message(sub.as_ref(), pub_arg, pendings)
                    .await
                    .map_err(|_| NError::new(ERROR_CONNECTION_CLOSED))?;
            }
        }
        Ok(())
    }
    ///消息格式
    ///```
    /// MSG <subject> <sid> <size>\r\n
    /// <message>\r\n
    /// ```
    async fn send_message(
        &self,
        sub: &Subscription,
        pub_arg: &PubArg<'_>,
        pendings: &mut BTreeSet<ClientMessageSenderWrapper>,
    ) -> std::io::Result<()> {
        let mut msg_sender = sub.msg_sender.lock().await;
        if let Some(ref mut msg_buf) = msg_sender.msg_buf {
            let id = msg_sender.deref() as *const ClientMessageSender as usize;
            let msg_buf = msg_sender.msg_buf.as_mut().unwrap();

            msg_buf.extend_from_slice("MSG ".as_bytes());
            msg_buf.extend_from_slice(sub.subject.as_bytes());
            msg_buf.extend_from_slice(" ".as_bytes());
            msg_buf.extend_from_slice(sub.sid.as_bytes());
            msg_buf.extend_from_slice(" ".as_bytes());
            msg_buf.extend_from_slice(pub_arg.size_buf.as_bytes());
            msg_buf.extend_from_slice("\r\n".as_bytes());
            msg_buf.extend_from_slice(pub_arg.msg); //经测试,如果这里不使用缓存,而是多个await,性能会大幅下降.
            msg_buf.extend_from_slice("\r\n".as_bytes());
            pendings.insert(ClientMessageSenderWrapper(sub.msg_sender.clone(), id));
        }
        Ok(())
    }
    /* async fn send_message2(sub: Arc<Subscription>, msg: Arc<Vec<u8>>) -> std::io::Result<()> {
        let mut msg_sender = sub.msg_sender.lock().await;
        let msg_buf = msg_sender.msg_buf.take().expect("must have");
        let writer = &mut msg_sender.writer;
        let mut buf = msg_buf.writer();
        buf.write("MSG ".as_bytes())?;
        buf.write(sub.subject.as_bytes())?;
        buf.write(" ".as_bytes())?;
        buf.write(sub.sid.as_bytes())?;
        buf.write(" ".as_bytes())?;
        write!(buf, "{}", msg.len())?;
        buf.write("\r\n".as_bytes())?;
        buf.write(msg.as_slice())?; //经测试,如果这里不使用缓存,而是多个await,性能会大幅下降.
        buf.write("\r\n".as_bytes())?;
        let mut msg_buf = buf.into_inner();
        writer.write_all(msg_buf.bytes()).await?;
        //        writer.flush().await?; 暂不需要flush,因为没有使用BufWriter
        msg_buf.clear();
        msg_sender.msg_buf = Some(msg_buf);
        Ok(())
    }*/
}
#[cfg(test)]
pub mod test_helper {
    use super::*;
    use lazy_static::lazy_static;
    lazy_static! {
        static ref SENDER: Arc<Mutex<ClientMessageSender>> = {
           let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        //    let port = l.local_addr().unwrap().port();
        let conn = std::net::TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();

        let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .unwrap();

        rt.spawn(async move {
            println!("tokio spawned");
            let conn = tokio::net::TcpStream::from_std(conn).unwrap();
            let (_, writer) = tokio::io::split(conn);
            println!("send start");
            let _=tx.send(writer);
            println!("send complete")
        });
        let writer = rx.recv().unwrap();
          Arc::new(Mutex::new(ClientMessageSender::new(writer)))
        };
    }
    #[cfg(test)]
    pub fn new_test_tcp_writer() -> Arc<Mutex<ClientMessageSender>> {
        SENDER.clone()
    }
}
use std::cmp::Ordering;
use std::ops::Deref;
#[cfg(test)]
pub use test_helper::new_test_tcp_writer;

#[cfg(test)]
mod tests {
    use super::*;
    extern crate test;
    use std::io::Write;
    use test::Bencher;

    #[test]
    fn test() {}
    #[test]
    fn test_rng() {
        for _ in 0..10 {
            let mut r = rand::rngs::StdRng::from_entropy();
            println!("next={}", r.next_u32());
        }
    }
    #[test]
    fn test_bytes() {
        let mut buf = Vec::with_capacity(100);
        buf.extend_from_slice("hello".as_bytes());
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.capacity(), 100);
        buf.clear();
        assert_eq!(buf.capacity(), 100);
        assert_eq!(buf.len(), 0);
    }
    #[bench]
    fn bench_gen_rng(b: &mut Bencher) {
        b.iter(|| {
            let r = rand::rngs::StdRng::from_entropy();
            drop(r);
        });
    }
}
