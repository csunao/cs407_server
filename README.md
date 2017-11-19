API
---

(subject to change...)

```c
void* server_start();
void server_send(void* handle, uint8_t client_id, uint8_t* data_ptr, size_t data_len);
size_t server_poll(void* handle, uint8_t** data_ptr);
void server_dealloc(void* handle, uint8_t data_ptr, size_t data_len);
void server_close(void* handle);
```

Packet Structure
----------------

You have to use this structure or your packets will not be recognised. Listed in order they will be read. Numbers are sent big endian.

Header:
```
32 bits: protocol_id (currently 0xFEEFBAAB)
8 bits: packet_type (see src/packet.rs)
```

Followed by either nothing or a structure dependent on packet_type:

If packet type is Confirm:
```
8 bits: client_id
```

If packet type is Payload:
```
8 bits: payload_size (in bytes)
8*payload_size bits: payload (a load of bytes you have to make sense of)
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

keepalive packets, disconnect packets, limit number of connections, sequences and a ton of other stuff

also a lot of cleaning up lol
