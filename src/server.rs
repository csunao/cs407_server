extern crate time;

use std::net::{UdpSocket, SocketAddr};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::collections::{HashMap, VecDeque};

use packet::*;
use connection::*;

pub const PROTOCOL_ID: u32 = 0xFEEFBAAB;

pub type ConnId = u8;
enum Job {
    Send(ConnId, Vec<u8>),
    SendAll(Vec<u8>),
    Disconnect(ConnId)
}

pub struct Server {
    socket: UdpSocket,
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    connections: Mutex<HashMap<ConnId, Connection>>,
    queue: Mutex<VecDeque<Vec<u8>>>,
    jobs: Mutex<VecDeque<Job>>,
    address_to_id: Mutex<HashMap<SocketAddr, ConnId>>
}

impl Server {
    pub fn new(address: String) -> Arc<Server> {
        let (sender, receiver) = mpsc::channel();
        let instance = Arc::new(Server {
            socket: UdpSocket::bind(address).unwrap(),
            threads: Mutex::new(Vec::new()),
            connections: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            jobs: Mutex::new(VecDeque::new()),
            address_to_id: Mutex::new(HashMap::new())
        });

        let listener_ref = instance.clone();
        let listener_sender = sender.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::listen(listener_ref, listener_sender); }
        ));

        let worker_ref = instance.clone();
        let worker_sender = sender.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::work(worker_ref, worker_sender); }
        ));

        let _ = receiver.recv();
        let _ = receiver.recv();

        instance
    }

    pub fn close(&self) {
        //for thread in self.threads.lock().unwrap().into_iter() {
            //thread.join().unwrap();
        //}
    }

    fn send(&self, recipient: ConnId, bytes: Vec<u8>) {
        match self.connections.lock().unwrap().get_mut(&recipient) {
            Some(ref mut conn) => {
                conn.send(&Packet::Payload(PayloadPacket { bytes }),
                          &self.socket, 0.0).unwrap();
            },
            None => panic!("Error: Trying to send msg to invalid conn
                              {:?}", recipient)
        }
    }

    fn send_all(&self, bytes: Vec<u8>) {
        for (_, conn) in self.connections.lock().unwrap().iter_mut() {
            conn.send(&Packet::Payload(PayloadPacket { bytes: bytes.clone() }),
                      &self.socket, 0.0).unwrap();
        }
    }

    pub fn poll(&self) -> Option<Vec<u8>> {
        self.queue.lock().unwrap().pop_front()
    }

    pub fn add_send_job(&self, recipient: ConnId, bytes: Vec<u8>) {
        self.jobs.lock().unwrap().push_back(Job::Send(recipient, bytes));
    }

    pub fn add_send_all_job(&self, bytes: Vec<u8>) {
        self.jobs.lock().unwrap().push_back(Job::SendAll(bytes));
    }

    fn disconnect(&self, client: ConnId) {
        let mut conns = self.connections.lock().unwrap();

        let address = {
            let connection = conns.get_mut(&client).unwrap();
            let packet = Packet::Disconnect;
            for _ in 0..10 {
                let _ = connection.send(&packet, &self.socket, time::precise_time_s());
            }
            connection.get_address()
        };

        conns.remove(&client);
        self.address_to_id.lock().unwrap().remove(&address);
        self.queue.lock().unwrap().push_back(vec![0, client]);
    }

    fn perform_job(&self) {
        let job = {
            let mut locked_jobs = self.jobs.lock().unwrap();
            if locked_jobs.len() == 0 { return };
            locked_jobs.pop_front().unwrap()
        };

        match job {
            Job::Send(recipient, bytes) => {
                self.send(recipient, bytes);
            },
            Job::SendAll(bytes) => {
                self.send_all(bytes);
            },
            Job::Disconnect(client) => {
                self.disconnect(client);
            }
        }
    }

    // processes messages
    fn listen(this: Arc<Server>, sender: mpsc::Sender<()>) {
        let socket = this.socket.try_clone().unwrap();
        let _ = sender.send(());
        let mut id: u8 = 1; // ids start at 1 xd

        let mut buf = [0; 256];
        loop {
            let (_, address) = socket.recv_from(&mut buf).unwrap();

            let mut conns = this.connections.lock().unwrap();

            let mut address_to_id = this.address_to_id.lock().unwrap();
            if !address_to_id.contains_key(&address) {
                let (_, packet) = decode(&buf[..], PROTOCOL_ID).unwrap();
                if let Packet::Connect = packet {
                    conns.insert(
                        id,
                        Connection::new(&address, id, PROTOCOL_ID, time::precise_time_s())
                    );
                    let connection = conns.get_mut(&id).unwrap();
                    address_to_id.insert(address, id);
                    let _ = connection.send(
                        &Packet::Confirm(ConfirmPacket { client_id: id }),
                        &socket, 0.0
                    );
                    id += 1;
                }
                
                continue
            };

            let packet = {
                let connection = conns.get_mut(address_to_id.get(&address).unwrap()).unwrap();
                connection.recv(&buf, time::precise_time_s()).unwrap()
            };

            match packet {
                Packet::Connect => {
                    // Resend confirmation, must have been lost
                    let connection = conns.get_mut(address_to_id.get(&address).unwrap()).unwrap();
                    let client_id = connection.get_id();
                    let _ = connection.send(
                        &Packet::Confirm(ConfirmPacket { client_id }),
                        &socket, 0.0
                    );
                },

                Packet::Payload(payload) => {
                    this.queue.lock().unwrap().push_back(payload.bytes);
                },

                Packet::KeepAlive => {
                    /* dun */
                },

                Packet::Disconnect => {
                    let client_id = {
                        let connection = conns.get_mut(address_to_id.get(&address).unwrap()).unwrap();
                        let packet = Packet::Disconnect;
                        for _ in 0..10 {
                            let _ = connection.send(&packet, &this.socket, time::precise_time_s());
                        }
                        connection.get_id()
                    };

                    conns.remove(&client_id);
                    address_to_id.remove(&address);
                    this.queue.lock().unwrap().push_back(vec![0, client_id]);
                    //this.jobs.lock().unwrap().push_back(Job::Disconnect(client_id));
                },

                msg => {
                    panic!("Received invalid msg ({:?})", msg);
                }
            }
        }
    }

    // Aiming for 64 tick rate?
    fn work(this: Arc<Server>, sender: mpsc::Sender<()>) {
        let _ = sender.send(());
        loop {
            for (id, conn) in this.connections.lock().unwrap().iter_mut() {
                match conn.tick(&this.socket, time::precise_time_s()).unwrap() {
                    UpdateResult::Expired => {
                        this.jobs.lock().unwrap().push_back(Job::Disconnect(*id));
                    }, _ => {}
                }
            }

            while this.jobs.lock().unwrap().len() > 0 {
                this.perform_job();
            }

            thread::sleep(::std::time::Duration::from_millis(10));
        }
    }
}
