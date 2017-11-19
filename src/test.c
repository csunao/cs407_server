#include <stdint.h>
#include <stdio.h>

// To compile,
// gcc -std=c11 src/test.c -L./target/debug -lcs407_server -o main
// To run,
// LD_LIBRARY_PATH=target/debug/ ./main

// I haven't updated this file to use current API yet compiling it won't
// do anything good!!

typedef void* ServerH;

extern ServerH server_start(const char*);
extern void server_send(ServerH, uint8_t, int32_t);
extern int32_t server_poll(ServerH);
extern void server_close(ServerH);

int main() {
    ServerH handle = server_start("127.0.0.1:9001");
    int64_t result;
    for (int i = 0; i < 10; ++i) {
        printf("Polling\n");
        if ((result = server_poll(handle)) != -1)
            printf("Got: %d\n", result);
        server_send(handle, 0, i); // Send i to first connection
    }

    server_close(handle);
    return 0;
}
