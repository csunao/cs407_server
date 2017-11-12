#include <stdint.h>
#include <stdio.h>

typedef void* ThreadH;

extern ThreadH server_start();
extern int32_t server_poll(ThreadH);
extern void server_send(int32_t);
extern void server_close(ThreadH);

int main() {
    ThreadH handle = server_start();
    server_close(handle);
    int64_t result;
    for (int i = 0; i < 10; ++i) {
        printf("Polling\n");
        if ((result = server_poll(handle)) != -1)
            printf("Got: %d\n", result);
        server_send(i);
        server_close(handle);
    }

    server_close(handle);
    return 0;
}
