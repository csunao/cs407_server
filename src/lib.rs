use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time;
use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
use std::os::raw::c_char;

/* API
 * start()
 * send(msg) // sends msg to specific connection
 * poll() -> msg
 * close() */

/* Architecture
   One listener thread -- accepts connections, adds them to list.
   One worker thread. Loops through connection list, accepting messages
   from all. After prototype, should think about having a different architecture
   such as one thread per mobile connection or Node's architecture.
   When message is found, added to queue.
 */

type ServerHandle = *const Server;
type ConnId = u8;

pub struct Server {
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    connections: Mutex<HashMap<ConnId, Connection>>,
    queue: Mutex<VecDeque<Message>>,
    jobs: Mutex<VecDeque<Job>>
}

enum Job {
    Send(ConnId, Message)
}

#[derive(Debug)]
struct Connection {
    stream: TcpStream
}

#[derive(Debug)]
struct Message {
    contents: i32
}

impl Message {
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

    fn from_bytes(bytes: [u8; 4]) -> Message {
        assert_eq!(bytes.len(), 4);
        let mut contents: i32 = 0;
        for byte in &bytes[..] {
            contents <<= 8;
            contents += *byte as i32;
        }
        Message { contents }
    }
}

#[no_mangle]
pub extern "C"
fn server_start(address: *const c_char) -> ServerHandle {
    let address = unsafe {
        CStr::from_ptr(address).to_string_lossy().into_owned()
    };

    let instance = Server::new(address);
    thread::sleep(time::Duration::from_millis(50));

    // pass raw pointer to caller to act as handle
    Arc::into_raw(instance)
}

#[no_mangle]
pub extern "C"
fn server_poll(ptr_handle: ServerHandle) -> i32 {
    let handle = unsafe { &*ptr_handle };
    let lock = handle.queue.try_lock();
    if let Ok(mut queue) = lock {
        match queue.pop_front() {
            Some(msg) => msg.contents,
            None => -1
        }
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C"
fn server_send(handle: ServerHandle, recipient: u8, contents: i32) {
    let handle = unsafe { &*handle };
    let msg = Message { contents };
    handle.add_send_job(recipient, msg);
}

#[no_mangle]
pub extern "C"
fn server_close(handle: ServerHandle ) {
    thread::sleep(time::Duration::from_millis(10));
    //let handle = unsafe { Arc::from_raw(handle) };
    let handle = unsafe { &*handle };
    handle.close();
}

impl Server {
    fn new(address: String) -> Arc<Server> {
        let (sender, receiver) = mpsc::channel();
        let instance = Arc::new(Server {
            threads: Mutex::new(Vec::new()),
            connections: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            jobs: Mutex::new(VecDeque::new())
        });

        let listener_ref = instance.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::listen(listener_ref, address, sender); }
        ));

        let worker_ref = instance.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::work(worker_ref, receiver); }
        ));

        instance
    }

    fn add_send_job(&self, recipient: ConnId, msg: Message) {
        self.jobs.lock().unwrap().push_back(Job::Send(recipient, msg));
    }

    fn close(&self) {
        println!("Closing. Heheh.");
    }

    fn send(&self, recipient: ConnId, msg: Message) {
        match self.connections.lock().unwrap().get_mut(&recipient) {
            Some(ref mut conn) => {
                let _ = conn.stream.write(&msg.to_bytes());
            },
            None => println!("Error: Trying to send msg to invalid conn")
        }
    }

    fn listen(this: Arc<Server>,
              address: String,
              _sender: mpsc::Sender<Connection>) {
        println!("Listener thread spawned.");
        let listener = TcpListener::bind(address).unwrap();
        let mut id: u8 = 0;
        for stream in listener.incoming() {
            println!("Received connection");
            let stream = stream.unwrap();
            stream.set_nonblocking(true).expect("nonblocking failed");
            this.connections.lock().unwrap().insert(
                id, 
                Connection { stream }
            );

            println!("Added connection, id {}", id);
            id += 1;
        }
    }

    fn work(this: Arc<Server>, _receiver: mpsc::Receiver<Connection>) {
        println!("Worker thread spawned.");
        // Need to check for new connections every loop the worker does.
        loop {
            // TODO: Handle case where connection has been closed
            {
                let mut locked_connections = this.connections.lock().unwrap();
                let mut bytes: [u8; 4] = [0; 4];
                for (_, conn) in locked_connections.iter_mut() {
                    // TODO: Account for invalid messages
                    while let Ok(v) = conn.stream.read(&mut bytes) {
                        if v == 0 { break; }
                        let msg = Message::from_bytes(bytes);
                        this.queue.lock().unwrap().push_back(msg);
                    }
                }
            }

            {
                let mut locked_jobs = this.jobs.lock().unwrap();
                while locked_jobs.len() > 0 {
                    match locked_jobs.pop_front().unwrap() {
                        Job::Send(recipient, msg) => this.send(recipient, msg)
                        //_ => {}
                    }
                }
            }


            thread::sleep(time::Duration::from_millis(1));
        }
    }
}


#[cfg(test)]
mod tests {
    use std::io::prelude::*;
    use std::net::TcpStream;
    use std::thread;
    use std::time;
    use std::ffi::CString;

    use Message;
    use server_start;
    use server_send;
    use server_poll;
    use server_close;

    #[test]
    fn mobile_to_unity() {
        let address = "127.0.0.1:9069";
        let address_cstr = CString::new(address).unwrap();
        let handle = server_start(address_cstr.as_ptr());
        thread::sleep(time::Duration::from_millis(100));
        let mut stream = TcpStream::connect(address).unwrap();

        let val_limit = 50;
        let vals: Vec<i32> = (0..val_limit).collect();
        for val in vals.iter() {
            let msg = Message { contents: *val };
            let _ = stream.write(&msg.to_bytes());
        }

        thread::sleep(time::Duration::from_millis(100));

        let mut results = Vec::new();
        let mut result: i32 = 0;
        while result != -1 {
            result = server_poll(handle);
            results.push(result);
        }
        results.pop();
        server_close(handle);
        assert_eq!(vals, results);
    }

    #[test]
    fn unity_to_mobile() {
        let address = "127.0.0.1:9070";
        let address_cstr = CString::new(address).unwrap();
        let handle = server_start(address_cstr.as_ptr());
        thread::sleep(time::Duration::from_millis(100));
        let mut stream = TcpStream::connect(address).unwrap();
        stream.set_nonblocking(true).expect("nonblocking failed");

        let val_limit = 50;
        let vals: Vec<i32> = (0..val_limit).collect();
        for val in vals.iter() {
            server_send(handle, 0, *val);
        }

        thread::sleep(time::Duration::from_millis(100));

        let mut bytes: [u8; 4] = [0; 4];
        let mut results = Vec::new();
        while let Ok(v) = stream.read(&mut bytes) {
            if v == 0 { break; }
            let msg = Message::from_bytes(bytes);
            results.push(msg.contents);
        }

        assert_eq!(vals, results);
    }
}
