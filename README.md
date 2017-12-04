Server API
----------

Starts server -- returns pointer used as handle in all other functions (except dealloc). Address format is e.g. 127.0.0.1:9000
```c
void* server_start(const char* address);
```

Send a message (a sequence of bytes) to all clients.
```c
void server_send(void* handle, uint8_t* data_ptr, size_t data_len);
```

Send a message (a sequence of bytes) to a specific client.
```c
void server_send_to(void* handle, uint8_t client_id, uint8_t* data_ptr, size_t data_len);
```

Poll the server for incoming messages. Result indicates length of message (in bytes) to be used to read that many bytes at `data_ptr`. All messages returned by this function must be deallocated by calling `server_dealloc` with the same pointer and length.
```c
size_t server_poll(void* handle, uint8_t** data_ptr);
```

Deallocate a message that was originally allocated on Rust side and so needs to be deallocated by Rust (all messages returned by `server_poll`).
```c
void server_dealloc(uint8_t* data_ptr, size_t data_len);
```

Close the server, deallocating resources and closing threads.
```c
void server_close(void* handle);
```

Client API
----------

Starts client -- returns pointer used as handle in all other functions (except dealloc). Address format is e.g. 127.0.0.1:9000, and ports can't be same for server & client in UDP
```c
void* client_connect(const char* server_address, const char* client_address);
```

Send a message (a sequence of bytes) to the server.
```c
void client_send(void* handle, uint8_t* data_ptr, size_t data_len);
```

Poll the client for incoming messages. Result indicates length of message (in bytes) to be used to read that many bytes at `data_ptr`. All messages returned by this function must be deallocated by calling `client_dealloc` with the same pointer and length.
```c
size_t client_poll(void* handle, uint8_t** data_ptr);
```

Deallocate a message that was originally allocated on Rust side and so needs to be deallocated by Rust (all messages returned by `client_poll`).
```c
void client_dealloc(uint8_t* data_ptr, size_t data_len);
```

returns this client's id
```c
uint8_t client_id(void* handle);
```

Disconnect cleanly from the server, sending packet to indicate disconnecting.
```c
void client_disconnect(void* handle);
```

Deallocate resources and close thread.
```c
void client_close(void* handle);
```

Compiling
---------

```
cargo build --release
```

 then lib will be in release/target/ (.so)

 i think, else google hehe

todo
----

limit number of connections, sequencing packets, possibly attempt to reconnect if disconnected through keepalive packets, cleaning up
