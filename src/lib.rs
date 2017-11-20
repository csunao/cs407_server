mod packet;
mod connection;

use std::net::UdpSocket;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
use std::os::raw::c_char;

use packet::*;
use connection::*;

const PROTOCOL_ID: u32 = 0xFEEFBAAB;

type ServerHandle = *const Server;
type ConnId = u8;
enum Job {
    Send(ConnId, Vec<u8>)
}

pub struct Server {
    socket: UdpSocket,
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    connections: Mutex<HashMap<ConnId, Connection>>,
    queue: Mutex<VecDeque<Vec<u8>>>,
    jobs: Mutex<VecDeque<Job>>
}

#[no_mangle]
pub extern "C"
fn server_start(address: *const c_char) -> ServerHandle {
    let address = unsafe {
        CStr::from_ptr(address).to_string_lossy().into_owned()
    };

    let instance = Server::new(address);

    // pass raw pointer to caller to act as handle
    Arc::into_raw(instance)
}

#[no_mangle]
pub extern "C"
fn server_poll(handle: ServerHandle, out_ptr: *mut (*mut u8)) -> u8 {
    let handle = unsafe { &*handle };
    let lock = handle.queue.lock();
    if let Ok(mut queue) = lock {
        match queue.pop_front() {
            Some(mut bytes) => {
                bytes.shrink_to_fit();
                let len = bytes.len() as u8;
                unsafe { *out_ptr = bytes.as_mut_ptr() };
                std::mem::forget(bytes);
                len
            }
            None => 0
        }
    } else {
        0
    }
}

#[no_mangle]
pub extern "C"
fn server_dealloc(ptr: *mut u8, len: u8) {
    let len = len as usize;
    drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
}

#[no_mangle]
pub extern "C"
fn server_send(handle: ServerHandle, recipient: ConnId,
               bytes: *const u8, byte_count: u32) {
    let handle = unsafe { &*handle };
    let byte_vec = unsafe { std::slice::from_raw_parts(bytes, byte_count as usize) }.to_vec();
    handle.add_send_job(recipient, byte_vec);
}

#[no_mangle]
pub extern "C"
fn server_close(handle: ServerHandle ) {
    let handle = unsafe { &*handle };
    handle.close();
}

impl Server {
    fn new(address: String) -> Arc<Server> {
        let (sender, receiver) = mpsc::channel();
        let instance = Arc::new(Server {
            socket: UdpSocket::bind(address).unwrap(),
            threads: Mutex::new(Vec::new()),
            connections: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            jobs: Mutex::new(VecDeque::new())
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

    fn close(&self) {
        //for thread in self.threads.lock().unwrap().into_iter() {
            //thread.join().unwrap();
        //}
    }

    fn send(&self, recipient: ConnId, bytes: Vec<u8>) {
        match self.connections.lock().unwrap().get_mut(&recipient) {
            Some(ref mut conn) => {
                conn.send(&Packet::Payload(PayloadPacket { bytes }),
                          &self.socket).unwrap();
            },
            None => println!("Error: Trying to send msg to invalid conn
                              {:?}", recipient)
        }
    }

    fn add_send_job(&self, recipient: ConnId, bytes: Vec<u8>) {
        self.jobs.lock().unwrap().push_back(Job::Send(recipient, bytes));
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
            }
        }
    }

    // processes messages
    fn listen(this: Arc<Server>, sender: mpsc::Sender<()>) {
        let socket = this.socket.try_clone().unwrap();
        let mut address_to_id = HashMap::new();
        let _ = sender.send(());
        let mut id: u8 = 0;

        let mut buf = [0; 256];
        loop {
            let (_, address) = socket.recv_from(&mut buf).unwrap();

            let mut conns = this.connections.lock().unwrap();

            if !address_to_id.contains_key(&address) {
                let (_, packet) = decode(&buf[..], PROTOCOL_ID).unwrap();
                if let Packet::Connect = packet {
                    println!("New connection -- sending confirm to {:?}", id);
                    conns.insert(
                        id,
                        Connection::new(&address, id, PROTOCOL_ID)
                    );
                    let connection = conns.get_mut(&id).unwrap();
                    address_to_id.insert(address, id);
                    id += 1;
                    let _ = connection.send(
                        &Packet::Confirm(ConfirmPacket { client_id: id }),
                        &socket
                    );
                }
                
                continue
            };

            let connection = conns.get_mut(address_to_id.get(&address).unwrap()).unwrap();

            let packet = connection.recv(&buf).unwrap();

            match packet {
                Packet::Connect => {
                    // Resend confirmation, must have been lost
                    let client_id = connection.get_id();
                    let _ = connection.send(
                        &Packet::Confirm(ConfirmPacket { client_id }),
                        &socket
                    );
                },

                Packet::Payload(payload) => {
                    println!("Got a payload!!");
                    println!("{:?}", &payload);
                    this.queue.lock().unwrap().push_back(payload.bytes);
                },

                Packet::Disconnect => {
                    /* remove connection */
                },

                msg => {
                    println!("Received invalid msg ({:?})", msg);
                }
            }
        }
    }

    // Aiming for 64 tick rate?
    fn work(this: Arc<Server>, sender: mpsc::Sender<()>) {
        let _ = sender.send(());
        loop {
            this.perform_job();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::thread;
    use std::time;
    use std::ffi::CString;
    use std::ptr;

    use server_start;
    use server_send;
    use server_poll;
    use server_close;

    use PROTOCOL_ID;

    use packet::*;

    fn connect(server: &str, client: &str) -> UdpSocket {
        let socket = UdpSocket::bind(client).unwrap();
        let _ = socket.connect(server);
        let mut buf = [0; 6];
        let _ = encode(&mut buf[..], PROTOCOL_ID, &Packet::Connect);

        let _ = socket.send(&buf[..]);
        let _ = socket.recv(&mut buf);
        let (_, response) = decode(&buf[..], PROTOCOL_ID).unwrap();
        assert_eq!(response.get_type_id(), PACKET_CONFIRM);

        match response {
            Packet::Confirm(ref ins) => {
                println!("Connected to server as client {}", &ins.client_id);
                socket
            },
            Packet::Deny => panic!("connection denied"),
            _ => panic!("invalid response to connect")
        }
    }

    #[test]
    fn packet_header() {
        let bytes = [0, 0, 0, 69, 0];
        let (seq, packet) = decode(&bytes[..], 69).unwrap();
        println!("packet: {:?}, {:?}", seq, packet);

    }

    #[test]
    fn udp_test() {
        let port = 9420;
        let s_addr = format!("{}:{}", "127.0.0.1", port.to_string());
        let s_addr_cstr = CString::new(&s_addr[..]).unwrap();
        let handle = server_start(s_addr_cstr.as_ptr());

        let client = connect(&s_addr[..], "127.0.0.1:9421");

        let other_sent_data = vec![96, 69];
        let sent_data = vec![42, 24];
        let sent_packet = Packet::Payload(PayloadPacket { bytes: sent_data.clone() });
        let mut sent_packet_bytes = [0; 5+1+2];
        encode(&mut sent_packet_bytes[..], PROTOCOL_ID, &sent_packet).unwrap();

        println!("zzz sending {:?}", &sent_packet_bytes[..]);
        client.send(&sent_packet_bytes[..]).unwrap();

        let bytes_ptr = other_sent_data.as_ptr();
        let bytes_count = other_sent_data.len() as u32;
        server_send(handle, 0, bytes_ptr, bytes_count);

        thread::sleep(time::Duration::from_millis(100));

        let mut ptr: *mut u8 = ptr::null_mut();
        let len = server_poll(handle, &mut ptr as *mut *mut u8) as usize;
        assert!(len != 0);

        let data = unsafe { Vec::from_raw_parts(ptr, len, len) };
        assert_eq!(data, sent_data);
        drop(data);

        let mut buf = [0; 256];
        client.recv_from(&mut buf[..]).unwrap();
        let (_, packet) = decode(&buf[..], PROTOCOL_ID).unwrap();
        println!("received {:?}", packet);

        server_close(handle);
    }
}
