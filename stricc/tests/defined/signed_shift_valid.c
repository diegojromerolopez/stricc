// Test: signed right shift is defined (arithmetic shift on this platform)
// Expected: clean exit

#include <stdio.h>

int main(void) {
    int x = -8;
    // Arithmetic right shift: -8 >> 1 should be -4
    int y = x >> 1;
    if (y != -4) {
        return 1;
    }
    // Positive right shift
    int a = 16;
    int b = a >> 2; // 4
    if (b != 4) {
        return 1;
    }
    return 0;
}
