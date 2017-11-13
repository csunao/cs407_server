#include <stdint.h>
#include <stdio.h>

typedef void* ServerH;

extern ServerH server_start(const char*);
extern void server_send(ServerH, uint8_t, int32_t);
extern int32_t server_poll(ServerH);
extern void server_close(ServerH);

int main() {
    ServerH handle = server_start("127.0.0.1:9001");
    server_close(handle);
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
