use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::VecDeque;

use packet::{self, Packet, PacketError};
//use connection::*;

pub struct Client {
    socket: UdpSocket,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
    queue: Mutex<VecDeque<Vec<u8>>>,
    protocol_id: u32,
    client_id: Mutex<u8>
}

#[derive(Debug)]
pub enum ClientError {
    ConnectionDenied,
    UnexpectedResponse
}

impl Client {
    pub fn new(server: String, client: String, protocol_id: u32) -> Result<Arc<Client>, ClientError> {
        let socket = UdpSocket::bind(client).unwrap();
        let _ = socket.connect(server);
        let mut buf = [0; 6];
        let _ = packet::encode(&mut buf[..], protocol_id, &Packet::Connect);
        let _ = socket.send(&buf[..]);
        let _ = socket.recv(&mut buf);
        let (_, response) = packet::decode(&buf[..], protocol_id).unwrap();

        let instance = Arc::new(Client {
            socket,
            thread: Mutex::new(None),
            queue: Mutex::new(VecDeque::new()),
            protocol_id,
            client_id: Mutex::new(100)
        });

        let thread_ref = Arc::clone(&instance);
        let thread = thread::spawn(move || {
            Client::run(thread_ref);
        });

        *(instance.thread.lock().unwrap()) = Some(thread);

        match response {
            Packet::Confirm(ref ins) => {
                *(instance.client_id.lock().unwrap()) = ins.client_id;
                Ok(instance)
            },
            Packet::Deny => Err(ClientError::ConnectionDenied),
            _ => Err(ClientError::UnexpectedResponse)
        }
    }

    pub fn send(&self, bytes: Vec<u8>) -> Result<usize, PacketError> {
        let mut buf = [0; 256];
        let packet = Packet::Payload(packet::PayloadPacket { bytes });
        let len = packet::encode(&mut buf, self.protocol_id, &packet)?;
        self.socket.send(&buf[..len])?;
        Ok(len)
    }

    pub fn poll(&self) -> Option<Vec<u8>> {
        self.queue.lock().unwrap().pop_front()
    }

    pub fn run(this: Arc<Client>) {
        let mut buf = [0; 256];
        loop {
            this.socket.recv_from(&mut buf[..]).unwrap();
            let (_, packet) = packet::decode(&buf[..], this.protocol_id).unwrap();

            match packet {
                Packet::KeepAlive => {
                    let len = packet::encode(&mut buf[..], this.protocol_id, &Packet::KeepAlive).unwrap();
                    this.socket.send(&buf[..len]).unwrap();
                },

                Packet::Disconnect => {
                    let msg: Vec<u8> = vec![
                        0xD, 0xE, 0xA, 0xD,
                        0xD, 0xE, 0xA, 0xD
                    ];

                    this.queue.lock().unwrap().push_back(msg);
                },

                Packet::Payload(payload) => {
                    this.queue.lock().unwrap().push_back(payload.bytes);
                },
                _ => ()
            }
        }
    }

    pub fn get_id(&self) -> u8 {
        *self.client_id.lock().unwrap()
    }

    pub fn disconnect(&self) {
        let mut buf = [0; 256];
        let len = packet::encode(&mut buf[..], self.protocol_id, &Packet::Disconnect).unwrap();

        for _ in 0..10 {
            self.socket.send(&buf[..len]).unwrap();
        }
    }
}
