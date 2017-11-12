use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time;

/* API
 * start()
 * send(msg)
 * poll() -> msg
 * close() */

/* Architecture
   One listener thread -- accepts connections, adds them to list.
   One worker thread. Loops through connection list, accepting messages
   from all. After prototype, should think about having a different architecture
   such as one thread per mobile connection or Node's architecture.
   When message is found, added to queue.
 */

pub struct Server {
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    connections: Mutex<Vec<Connection>>,
    queue: Mutex<Vec<Message>>
}

impl Drop for Server {
    fn drop(&mut self) {
        println!("Server getting dropped *************");
    }
}

type ServerHandle = *const Server;

#[derive(Debug)]
struct Connection {
    id: u8, // I can't imagine us connecting over 256 mobile devices.
    stream: TcpStream
}

#[derive(Debug)]
struct Message {
    id: u8,
    contents: i32
}

impl Message {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.id);
        let contents_bytes: [u8; 4] = unsafe {
            std::mem::transmute(self.contents)
        };

        for byte in contents_bytes.into_iter().rev() {
            bytes.push(*byte);
        }
        bytes
    }

    fn from_bytes(bytes: [u8; 5]) -> Message {
        assert_eq!(bytes.len(), 5);
        let id = bytes[0];
        let mut contents: i32 = 0;
        for byte in &bytes[1..] {
            contents <<= 8;
            contents += *byte as i32;
        }
        Message { id, contents }
    }
}

#[no_mangle]
pub extern "C"
fn server_start() -> ServerHandle {
    let instance = Server::new();
    thread::sleep(time::Duration::from_millis(50));

    println!("Counts: {}, {}", Arc::strong_count(&instance),
    Arc::weak_count(&instance));
    // pass raw pointer to caller to act as handle
    Arc::into_raw(instance)
}

#[no_mangle]
pub extern "C"
fn server_poll(ptr_handle: ServerHandle ) -> i32 {
    let handle = unsafe { &*ptr_handle };
    let result = {
        let lock = handle.queue.try_lock();
        if let Ok(mut queue) = lock {
            match queue.pop() {
                Some(msg) => msg.contents,
                None => -1
            }
        } else {
            -1
        }
        //lock.unwrap().pop().unwrap().contents

        /*
        match lock.unwrap().pop() {
            Some(v) => v.contents,
            None => -1
        }
        */
    };

    result
}

#[no_mangle]
pub extern "C"
fn server_send(contents: i32) {
    let mut stream = TcpStream::connect("127.0.0.1:9001").unwrap();
    let msg = Message { id: 2, contents };
    let _ = stream.write(&msg.to_bytes());
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
    fn new() -> Arc<Server> {
        let (sender, receiver) = mpsc::channel();
        let instance = Arc::new(Server {
            threads: Mutex::new(Vec::new()),
            connections: Mutex::new(Vec::new()),
            queue: Mutex::new(Vec::new())
        });

        let listener_ref = instance.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::listen(listener_ref, sender); }
        ));

        let worker_ref = instance.clone();
        instance.threads.lock().unwrap().push(thread::spawn(
            move || { Server::work(worker_ref, receiver); }
        ));

        instance
    }

    fn close(&self) {
        println!("Closing");
    }

    fn listen(this: Arc<Server>, _sender: mpsc::Sender<Connection>) {
        println!("Listener thread spawned.");
        let listener = TcpListener::bind("127.0.0.1:9001").unwrap();
        for stream in listener.incoming() {
            println!("Received connection");
            this.connections.lock().unwrap().push(Connection {
                id: 0,
                stream: stream.unwrap()
            });

            println!("Added connection");
        }
    }

    fn work(this: Arc<Server>, _receiver: mpsc::Receiver<Connection>) {
        println!("Worker thread spawned.");
        // Need to check for new connections every loop the worker does.
        loop {
            /*
            match receiver.try_recv() { Ok(v) => {
                println!("Worker received: {:?}", v);
                connections.push(v);
            }, Err(_) => { /* no new connections */ } }
            */

            // TODO: Handle case where connection has been closed
            {
                let mut locked_connections = this.connections.lock().unwrap();
                for conn in locked_connections.iter_mut() {
                    // TODO: May have received more than one message at a time
                    // This should always be a multiple of 5 bytes atm, so read
                    // 5 bytes at a time
                    let mut bytes: [u8; 5] = [0; 5];
                    while conn.stream.read(&mut bytes).unwrap() != 0 {
                        let msg = Message::from_bytes(bytes);
                        this.queue.lock().unwrap().push(msg);
                        println!("Added message to queue");
                    }
                }
            }

            // 60 ticks/second or so right now
            thread::sleep(time::Duration::from_millis(16));
        }
    }
}

/*
fn handle_request(stream: TcpStream) {
    println!("Received: ");
    for byte in stream.bytes() {
        println!("{}", byte.unwrap());
    }
}
*/




#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
