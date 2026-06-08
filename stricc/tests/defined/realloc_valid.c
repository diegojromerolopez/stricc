// Test: realloc properly updates shadow metadata
// Expected: clean exit (bounds tracking works on reallocated pointer)

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    int *buf = (int *)malloc(4 * sizeof(int));
    if (!buf) return 1;

    buf[0] = 10;
    buf[1] = 20;
    buf[2] = 30;
    buf[3] = 40;

    // Grow the buffer
    int *bigger = (int *)realloc(buf, 8 * sizeof(int));
    if (!bigger) {
        free(buf);
        return 1;
    }

    bigger[4] = 50;
    bigger[5] = 60;
    bigger[6] = 70;
    bigger[7] = 80;

    int sum = 0;
    for (int i = 0; i < 8; i++) {
        sum += bigger[i];
    }
    // 10+20+30+40+50+60+70+80 = 360
    free(bigger);

    if (sum != 360) return 1;
    return 0;
}
