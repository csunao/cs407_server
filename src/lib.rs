use std::net::{UdpSocket, SocketAddr};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
use std::os::raw::c_char;

/* API
 * start() -> ServerHandle
 * send(ServerHandle, ConnId, MsgType) // sends msg to specific connection
 * send_all(ServerHandle, MsgType)     // sends msg to all connected
 * poll(ServerHandle) -> Message
 * close(ServerHandle)
 */

type ServerHandle = *const Server;
type ConnId = i8;
type MsgType = i32;

enum Job {
    Send(ConnId, Message)
}

#[derive(Debug)]
struct Connection {
    address: SocketAddr
}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Message {
    id: ConnId,
    contents: MsgType
}

impl Message {
    fn new(id: ConnId, contents: MsgType) -> Message {
        Message { id, contents }
    }

    fn from_contents (contents: MsgType) -> Message {
        Message::new(-1, contents)
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let contents_bytes: [u8; 4] = unsafe {
            std::mem::transmute(self.contents)
        };

        for byte in contents_bytes.into_iter().rev() {
            bytes.push(*byte);
        }
        bytes
    }

    fn from_bytes(id: ConnId, bytes: &[u8]) -> Message {
        assert_eq!(bytes.len(), 4);
        let mut contents: MsgType = 0;
        for byte in &bytes[..] {
            contents <<= 8;
            contents += *byte as MsgType;
        }

        Message::new(id, contents)
    }
}

pub struct Server {
    socket: UdpSocket,
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    connections: Mutex<HashMap<ConnId, Connection>>,
    queue: Mutex<VecDeque<Message>>,
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
fn server_poll(handle: ServerHandle) -> Message {
    let handle = unsafe { &*handle };
    let lock = handle.queue.lock();
    if let Ok(mut queue) = lock {
        match queue.pop_front() {
            Some(msg) => msg,
            None => Message::new(-1, -1)
        }
    } else {
        Message::new(-1, -1)
    }
}

#[no_mangle]
pub extern "C"
fn server_send(handle: ServerHandle, recipient: ConnId, message: MsgType) {
    let handle = unsafe { &*handle };
    handle.add_send_job(recipient, Message::new(recipient, message));
}

#[no_mangle]
pub extern "C"
fn server_send_all(handle: ServerHandle, message: MsgType) {
    let handle = unsafe { &*handle };
    handle.add_send_jobs(message);
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
    }

    fn send(&self, recipient: ConnId, msg: Message) {
        match self.connections.lock().unwrap().get_mut(&recipient) {
            Some(ref conn) => {
                let _ = self.socket.send_to(&msg.to_bytes(), conn.address);
            },
            None => println!("Error: Trying to send msg to invalid conn {:?}", recipient)
        }
    }

    fn add_send_job(&self, recipient: ConnId, msg: Message) {
        self.jobs.lock().unwrap().push_back(Job::Send(recipient, msg));
    }

    fn add_send_jobs(&self, msg: MsgType) {
        for (id, _) in self.connections.lock().unwrap().iter() {
            self.jobs.lock().unwrap().push_back(Job::Send(*id, Message::new(*id, msg)));
        }
    }

    fn perform_job(&self) {
        let mut locked_jobs = self.jobs.lock().unwrap();
        while locked_jobs.len() > 0 {
            match locked_jobs.pop_front().unwrap() {
                Job::Send(recipient, msg) => self.send(recipient, msg)
                //_ => {}
            }
        }
    }

    // processes messages
    fn listen(this: Arc<Server>, sender: mpsc::Sender<()>) {
        let socket = this.socket.try_clone().expect("got fukked");
        let mut address_to_id = HashMap::new();
        let _ = sender.send(());
        let mut id = 0;

        let mut buf = [0; 5];
        loop {
            let (_, address) = socket.recv_from(&mut buf).expect("error?");
            match buf[0] {
                0 => {
                    println!("Received init -- sending ack to {:?}", id);
                    this.connections.lock().unwrap().insert(
                        id,
                        Connection { address }
                    );
                    address_to_id.insert(address, id);
                    id += 1;
                    let _ = socket.send_to(&[1], &address);
                },

                2 => {
                    let id = address_to_id.get(&address).unwrap();
                    let msg = Message::from_bytes(*id, &buf[1..]);
                    println!("Received {:?} from {:?}", msg.contents, address);
                    this.queue.lock().unwrap().push_back(msg);
                },

                msg => {
                    println!("Received invalid msg ({:?})", msg);
                }
            }
        }
    }

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
    use std::collections::HashMap;

    use Message;
    use MsgType;
    use ConnId;
    use server_start;
    use server_send;
    use server_poll;
    use server_close;
    use server_send_all;

    fn connect(server: &str, client: &str) -> UdpSocket {
        let socket = UdpSocket::bind(client).unwrap();
        let _ = socket.connect(server);
        let _ = socket.send(&[0]);
        let mut buf = [0; 1];
        let _ = socket.recv(&mut buf);
        assert_eq!(buf[0], 1);
        println!("acked with server successfully");

        socket
    }

    #[test]
    fn udp_test() {
        let mut port = 9420;
        let s_addr = format!("{}:{}", "127.0.0.1", port.to_string());
        let s_addr_cstr = CString::new(&s_addr[..]).unwrap();
        let handle = server_start(s_addr_cstr.as_ptr());

        let client_amt = 10;
        let mut clients = HashMap::new();
        for i in 0..client_amt {
            port += 1;
            let c_addr = format!("{}:{}", "127.0.0.1", port.to_string());
            clients.insert(i, connect(&s_addr[..], &c_addr[..]));
        }

        let mut sents: HashMap<ConnId, MsgType> = HashMap::new();
        for (i, _) in clients.iter() {
            let num = *i as MsgType;
            sents.insert(*i, num*num*num);
        }

        for (id, contents) in sents.iter() {
            let msg = Message::from_contents(*contents);
            let mut msg = msg.to_bytes();
            msg.insert(0, 2);
            let _ = clients.get(&id).unwrap().send(&msg[..]);
        }

        server_send(handle, 0, 239843);

        thread::sleep(time::Duration::from_millis(10));

        let mut bytes = [0; 4];
        let _ = clients.get(&0).unwrap().recv_from(&mut bytes);

        let mobile_result = Message::from_bytes(-1, &bytes[..]).contents;

        server_close(handle);

        let mut results = Vec::new();
        loop {
            match server_poll(handle) {
                Message { id: -1, contents: _ } => break,
                msg => results.push(msg)
            }
        }

        assert_eq!(mobile_result, 239843);

        for result in results.into_iter() {
            let original = Message::new(
                result.id, 
                *sents.get(&result.id).unwrap()
            );
            assert_eq!(result, original);
        }
    }
}
