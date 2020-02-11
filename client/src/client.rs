use crate::parser::Parser;
use crate::parser::*;
use bytes::buf::BufMutExt;
use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::*;
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};

type MessageHandler = Box<dyn FnMut(&[u8]) -> std::result::Result<(), ()> + Sync + Send>;
//#[derive(Debug)]
pub struct Client {
    addr: String,
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    msg_buf: Option<BytesMut>,
    pub stop: Option<oneshot::Sender<()>>,
    sid: u64,
    handler: Arc<Mutex<HashMap<String, MessageHandler>>>,
}

impl Client {
    pub async fn connect(addr: &str) -> std::io::Result<Client> {
        let conn = TcpStream::connect(addr).await?;
        let (reader, writer) = tokio::io::split(conn);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg_sender = Arc::new(Mutex::new(HashMap::new()));
        let writer = Arc::new(Mutex::new(writer));
        tokio::spawn(Self::receive_task(
            reader,
            rx,
            msg_sender.clone(),
            writer.clone(),
        ));
        return Ok(Client {
            addr: addr.to_string(),
            writer,
            stop: Some(tx),
            sid: 0,
            handler: msg_sender,
            msg_buf: Some(BytesMut::with_capacity(512)),
        });
    }
    async fn receive_task(
        mut reader: ReadHalf<TcpStream>,
        stop: oneshot::Receiver<()>,
        handler: Arc<Mutex<HashMap<String, MessageHandler>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    ) {
        use futures::*;
        let mut buf = [0 as u8; 512];
        let mut parser = Parser::new();
        let mut stop = stop.fuse();
        //        let mut _r: Result<usize>;
        loop {
            select! {
                            _=stop=>{
                            println!("client stoped.");
                            let r=writer.lock().await.shutdown().await;
                             if r.is_err() {
                                println!("receive_task err {:?}", r.unwrap_err());
                                return;
                            }
                            return;
                            },
                              r = reader.read(&mut buf[..]).fuse()=>{
                            if r.is_err() {
                                println!("receive_task err {:?}", r.unwrap_err());
                                return;
                            }
                            let r = r.unwrap();
                            if r == 0 {
                                println!("connection closed");
                                return;
                            }
                            let mut buf = &buf[0..r];
            //                println!("read buf len={},buf={}", r, unsafe {
            //                    std::str::from_utf8_unchecked(buf)
            //                });
                            loop {
                                let r = parser.parse(buf);
                                if r.is_err() {
                                    println!("msg error:{}", r.unwrap_err());
                                    let r=writer.lock().await.shutdown().await;
                                    if r.is_err() {
                                        println!("shutdown err {:?}",r);
                                    }
                                    return;
                                }
                                let (r, n) = r.unwrap();
                                if let ParseResult::MsgArg(ref msg) = r {
                                    if let Some(handler) = handler.lock().await.get_mut(msg.subject) {
                                        let r = handler(msg.msg);
                                        if r.is_err() {
                                            println!("handler error {:?}", r.unwrap_err());
                                            return;
                                        }
                                    } else {
                                        println!("receive msg on subject {}, not found receiver", msg.subject);
                                    }
                                    parser.clear_msg_buf();
                                } else if ParseResult::NoMsg == r {
                                    break; //NoMsg
                                }
            //                    println!("n={},buf len={}", n, buf.len());
                                if n == buf.len() {
                                    break;
                                }
                                buf = &buf[n..];
                            }
                        }
                             }
        }
    }
    //pub消息格式为PUB subject size\r\n{message}
    pub async fn pub_message(&mut self, subject: &str, msg: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let msg_buf = self.msg_buf.take().expect("must have");
        let mut writer = msg_buf.writer();
        writer.write("PUB ".as_bytes())?;
        writer.write(subject.as_bytes())?;
        //        write!(writer, subject)?;
        write!(writer, " {}\r\n", msg.len())?;
        writer.write(msg)?; //todo 这个需要copy么?最好别copy
        writer.write("\r\n".as_bytes())?;
        let mut msg_buf = writer.into_inner();
        let mut writer = self.writer.lock().await;
        writer.write(msg_buf.bytes()).await?;
        msg_buf.clear();
        self.msg_buf = Some(msg_buf);
        Ok(())
    }
    //批量pub,
    pub async fn pub_messages(&mut self, subject: &[&str], msg: &[&[u8]]) -> std::io::Result<()> {
        use std::io::Write;
        let msg_buf = BytesMut::new(); //  self.msg_buf.take().expect("must have");
        let mut writer = msg_buf.writer();
        for i in 0..subject.len() {
            writer.write("PUB ".as_bytes())?;
            writer.write(subject[i].as_bytes())?;
            //        write!(writer, subject)?;
            write!(writer, " {}\r\n", msg[i].len())?;
            writer.write(msg[i])?; //todo 这个需要copy么?最好别copy
            writer.write("\r\n".as_bytes())?;
        }
        let mut msg_buf = writer.into_inner();
        let mut writer = self.writer.lock().await;
        {
            let s = unsafe { std::str::from_utf8_unchecked(msg_buf.bytes()) };
            if s.find("JJP").is_some() {
                println!("send={}", s);
                std::process::exit(32);
            }
        }
        writer.write(msg_buf.bytes()).await?;
        //        msg_buf.clear();
        //        self.msg_buf = Some(msg_buf);
        Ok(())
    }
    //    type MessageHandler = Box<dyn Fn(&[u8]) -> Result<()> + Sync + Send >;
    //sub消息格式为SUB subject {queue} {sid}\r\n
    //可能由于rustc的bug,导致如果subject是&str,则会报错E0700,暂时使用String来替代
    pub async fn sub_message(
        &mut self,
        subject: String,
        queue: Option<String>,
        handler: MessageHandler,
    ) -> std::io::Result<()> {
        self.sid += 1;
        let mut writer = self.writer.lock().await;
        if let Some(q) = queue {
            writer
                .write(format!("SUB {} {} {}\r\n", subject, q, self.sid).as_bytes())
                .await?;
        } else {
            writer
                .write(format!("SUB {} {}\r\n", subject, self.sid).as_bytes())
                .await?;
        }
        //        let (tx, rx) = mpsc::unbounded_channel();
        self.handler
            .lock()
            .await
            .insert(subject.to_string(), handler);
        Ok(())
    }
    pub fn close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    struct A {
        a: String,
    }
    impl A {
        async fn test(
            &mut self,
            arg: &str,
            handler: Box<dyn Fn(&[u8]) -> std::result::Result<(), ()> + Send + Sync + '_>,
        ) {
        }
    }
    type MessageHandler = Box<dyn Fn(&[u8]) -> std::result::Result<(), ()> + Sync + Send>;
    async fn test2(handler: MessageHandler) {
        let args = "hello".to_string();
        handler(args.as_bytes());
    }
    fn print_hello(args: &[u8]) -> std::result::Result<(), ()> {
        println!("{:?}", args);
        Ok(())
    }
    #[test]
    fn test() {}
    #[tokio::main]
    #[test]
    async fn test_2() {
        test2(Box::new(print_hello)).await
    }

    use std::cell::Cell;

    trait Trait<'a> {}

    impl<'a, 'b> Trait<'b> for Cell<&'a u32> {}

    fn foo<'x, 'y>(x: Cell<&'x u32>) -> impl Trait<'y> + 'x
    where
        'x: 'y,
    {
        x
    }
}
