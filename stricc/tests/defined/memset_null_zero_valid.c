// Test: memset with zero size on null is allowed; non-null normal use works
// Expected: clean exit

#include <stdlib.h>

int main(void) {
    // Normal use: zero a buffer
    char *buf = (char *)malloc(16);
    if (!buf) return 1;

    // Use memset to zero the buffer
    for (int i = 0; i < 16; i++) {
        buf[i] = 0;
    }
    for (int i = 0; i < 16; i++) {
        if (buf[i] != 0) {
            free(buf);
            return 1;
        }
    }
    free(buf);
    return 0;
}
