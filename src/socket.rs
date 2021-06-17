pub struct Socket {
    socket: std::net::UdpSocket,
    clients: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<std::net::SocketAddr, std::time::SystemTime>>,
    >,
}

pub fn new(address: &str) -> Result<Socket, std::io::Error> {
    let socket = std::net::UdpSocket::bind(address)?;
    socket.set_read_timeout(Some(std::time::Duration::from_micros(100)))?;
    Ok(Socket {
        socket,
        clients: std::sync::Arc::new(std::sync::Mutex::new(Default::default())),
    })
}

impl Socket {
    fn remove_old_clients(
        &self,
    ) -> std::collections::HashMap<std::net::SocketAddr, std::time::SystemTime> {
        self.clients
            .lock()
            .unwrap()
            .drain_filter(|_client, time| {
                std::time::SystemTime::now()
                    .duration_since(*time)
                    .unwrap()
                    .as_secs()
                    > 10
            })
            .collect()
    }

    pub fn write(&self, data: &[u8]) {
        self.remove_old_clients();
        for client in self.clients.lock().unwrap().keys() {
            if let Err(error) = self.socket.send_to(data, client) {
                println!(
                    "Error while writing in UDP: {:?} for client: {}",
                    error, client
                );
            }
        }
    }

    pub fn read(&self) -> Vec<u8> {
        let mut buffer = vec![0; 4096];
        let mut data = vec![];
        while let Ok((size, client)) = self.socket.recv_from(&mut buffer) {
            self.clients
                .lock()
                .unwrap()
                .insert(client, std::time::SystemTime::now());
            data.extend_from_slice(&buffer[..size]);
        }
        return data;
    }
}
