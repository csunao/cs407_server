mod packet;
mod connection;
mod server;
mod client;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Arc;

use server::*;
use client::*;

type ServerHandle = *const Server;

// types
// client id should be size_t really..? nah, u8 is fine.

// Client API
// client_connect(server_address, client_address) -> handle --- done
// client_send(handle, bytes, len) --- done
// client_poll(handle, bytes) -> len --- done
// client_dealloc(bytes, len) --- done
// client_id(handle) -> u8 --- done
// client_disconnect(handle) --- done
// client_close(handle) --- done

#[no_mangle]
pub extern "C"
fn client_connect(server: *const c_char, client: *const c_char) -> *const Client {
    let server = unsafe { CStr::from_ptr(server).to_string_lossy().into_owned() };
    let client = unsafe { CStr::from_ptr(client).to_string_lossy().into_owned() };

    let instance = Client::new(server, client, PROTOCOL_ID).unwrap();
    Arc::into_raw(instance)
}

#[no_mangle]
pub extern "C"
fn client_send(handle: *const Client, bytes: *const u8, byte_count: usize) {
    let handle = unsafe { &*handle };
    let byte_vec = unsafe { std::slice::from_raw_parts(bytes, byte_count as usize) }.to_vec();
    let _ = handle.send(byte_vec);
}

#[no_mangle]
pub extern "C"
fn client_id(handle: *const Client) -> u8 {
    let handle = unsafe { &* handle };
    handle.get_id()
}

#[no_mangle]
pub extern "C"
fn client_poll(handle: *const Client, out_ptr: *mut (*mut u8)) -> usize {
    let handle = unsafe { &*handle };

    match handle.poll() {
        Some(mut bytes) => {
            bytes.shrink_to_fit();
            let len = bytes.len();
            unsafe { *out_ptr = bytes.as_mut_ptr() };
            std::mem::forget(bytes);
            len
        }
        None => 0
    }
}

#[no_mangle]
pub extern "C"
fn client_dealloc(ptr: *mut u8, len: u8) {
    let len = len as usize;
    drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
}

#[no_mangle]
pub extern "C"
fn client_disconnect(handle: *const Client) {
    let handle = unsafe { &*handle };
    handle.disconnect();
}

#[no_mangle]
pub extern "C"
fn client_close(handle: *const Client) {
    //let handle = unsafe { &*handle };
    drop(unsafe { Arc::from_raw(handle) });
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

    match handle.poll() {
        Some(mut bytes) => {
            bytes.shrink_to_fit();
            let len = bytes.len() as u8;
            unsafe { *out_ptr = bytes.as_mut_ptr() };
            std::mem::forget(bytes);
            len
        }
        None => 0
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
fn server_send(handle: ServerHandle, bytes: *const u8, byte_count: usize) {
    let handle = unsafe { &*handle };
    let byte_vec = unsafe { std::slice::from_raw_parts(bytes, byte_count as usize) }.to_vec();
    handle.add_send_all_job(byte_vec);
}

#[no_mangle]
pub extern "C"
fn server_send_to(handle: ServerHandle, recipient: ConnId,
               bytes: *const u8, byte_count: usize) {
    let handle = unsafe { &*handle };
    let byte_vec = unsafe { std::slice::from_raw_parts(bytes, byte_count as usize) }.to_vec();
    handle.add_send_job(recipient, byte_vec);
}

#[no_mangle]
pub extern "C"
fn server_close(handle: ServerHandle ) {
    {
        let handle = unsafe { &*handle };
        handle.close();
    }
    drop(unsafe { Arc::from_raw(handle) });
}

#[cfg(test)]
mod tests {
    extern crate time;

    use std::net::UdpSocket;
    use std::thread;
    use std::time::Duration;
    use std::ffi::CString;
    use std::ptr;

    use server_start;
    use server_send_to;
    use server_poll;
    use server_close;

    use client_connect;
    use client_send;
    use client_poll;
    use client_dealloc;
    use client_id;
    use client_disconnect;
    use client_close;

    use server::PROTOCOL_ID;

    use packet::*;

    #[test]
    fn client_test() {
        let (server, client) = {
            let mut port = 9422;
            let s_addr = format!("{}:{}", "127.0.0.1", port.to_string());
            let s_addr_cstr = CString::new(&s_addr[..]).unwrap();
            port += 1;
            let c_addr = format!("{}:{}", "127.0.0.1", port.to_string());
            let c_addr_cstr = CString::new(&c_addr[..]).unwrap();

            let server = server_start(s_addr_cstr.as_ptr());
            let client = client_connect(s_addr_cstr.as_ptr(), c_addr_cstr.as_ptr());
            (server, client)
        };

        let sent_data = vec![42, 24];
        let other_data = vec![96, 69];
        thread::sleep(Duration::from_millis(30));

        server_send_to(server, 1, sent_data.as_ptr(), sent_data.len());
        client_send(client, other_data.as_ptr(), other_data.len());

        thread::sleep(Duration::from_millis(30));

        let mut ptr: *mut u8 = ptr::null_mut();
        let len = client_poll(client, &mut ptr as *mut *mut u8) as usize;
        client_dealloc(ptr, len as u8);

        println!("we're client number {}", client_id(client));

        thread::sleep(Duration::from_millis(6000));

        loop {
            let mut ptr: *mut u8 = ptr::null_mut();
            let len = client_poll(client, &mut ptr as *mut *mut u8) as usize;
            let data = unsafe { Vec::from_raw_parts(ptr, len, len) };
            if len == 0 {
                break;
            }
            println!("client got {:?}", data);
        }

        loop {
            let mut ptr: *mut u8 = ptr::null_mut();
            let len = server_poll(server, &mut ptr as *mut *mut u8) as usize;
            let data = unsafe { Vec::from_raw_parts(ptr, len, len) };
            if len == 0 {
                break;
            }
            println!("server got {:?}", data);
        }

        client_disconnect(client);
        client_close(client);
    }
}
