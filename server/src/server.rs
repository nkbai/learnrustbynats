use crate::client::*;
use crate::simple_sublist::SubListTrait;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct Server<T: SubListTrait> {
    state: Arc<Mutex<ServerState<T>>>,
}
#[derive(Debug, Default)]
pub struct ServerState<T: SubListTrait> {
    clients: HashMap<u64, Arc<Mutex<ClientMessageSender>>>,
    pub sublist: T,
    pub gen_cid: u64,
}

impl<T: SubListTrait + Send + 'static> Server<T> {
    pub async fn start(self) -> Result<(), Box<dyn Error>> {
        let addr = "127.0.0.1:4222";
        let mut listener = TcpListener::bind(addr).await?;
        //go func(){}
        loop {
            let (conn, _) = listener.accept().await?;
            self.new_client(conn).await;
        }
    }
    async fn new_client(&self, conn: TcpStream) {
        let state = self.state.clone();
        let cid = {
            let mut state = state.lock().await;
            state.gen_cid += 1;
            state.gen_cid
        };
        let c = Client::process_connection(cid, state, conn);
        self.state.lock().await.clients.insert(cid, c);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
