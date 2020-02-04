use crate::parser::Parser;
use crate::parser::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::*;
use tokio::io::{self, AsyncWrite, AsyncWriteExt};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
type MessageHandler = Box<dyn Fn(&[u8]) -> Result<()> + Sync + Send>;
#[derive(Debug)]
pub struct Client {
    addr: String,
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    stop: oneshot::Sender<()>,
    sid: u64,
    handler: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>>,
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
            stop: tx,
            sid: 0,
            handler: msg_sender,
        });
    }
    async fn receive_task(
        mut reader: ReadHalf<TcpStream>,
        stop: oneshot::Receiver<()>,
        handler: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    ) {
        use tokio::io::AsyncWrite;
        use tokio::io::AsyncWriteExt;
        let mut buf = [0 as u8; 512];
        let mut parser = Parser::new();
        loop {
            let r = reader.read(&mut buf[..]).await;
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
            println!("read buf len={},buf={}", r, unsafe {
                std::str::from_utf8_unchecked(buf)
            });
            loop {
                let r = parser.parse(buf);
                if r.is_err() {
                    println!("msg error:{}", r.unwrap_err());
                    writer.lock().await.shutdown().await;
                    return;
                }
                let (r, n) = r.unwrap();
                if let ParseResult::MsgArg(ref msg) = r {
                    if let Some(handler) = handler.lock().await.get(msg.subject) {
                        let r = handler.send(msg.msg.to_vec());
                        if r.is_err() {
                            println!("handler error {}", r.unwrap_err());
                            return;
                        }
                    } else {
                        println!("receive msg on subject {}, not found receiver", msg.subject);
                    }
                    parser.clear_msg_buf();
                } else if ParseResult::NoMsg == r {
                    break; //NoMsg
                }
                println!("n={},buf len={}", n, buf.len());
                if n == buf.len() {
                    break;
                }
                buf = &buf[n..];
            }
        }
    }
    //pub消息格式为PUB subject size\r\n{message}
    pub async fn pub_message(&mut self, subject: &str, msg: &[u8]) -> std::io::Result<()> {
        let mut writer = self.writer.lock().await;
        writer.write("PUB ".as_bytes()).await?;
        writer.write(subject.as_bytes()).await?;
        writer
            .write(format!(" {}\r\n", msg.len()).as_bytes())
            .await?;
        writer.write(msg).await?;
        writer.write("\r\n".as_bytes()).await?;
        Ok(())
    }
    //sub消息格式为SUB subject {queue} {sid}\r\n
    pub async fn sub_message(
        &mut self,
        subject: &str,
        queue: Option<&str>,
        //        handler: MessageHandler,
    ) -> std::io::Result<mpsc::UnboundedReceiver<Vec<u8>>> {
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
        let (tx, rx) = mpsc::unbounded_channel();
        self.handler.lock().await.insert(subject.to_string(), tx);
        return Ok(rx);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
