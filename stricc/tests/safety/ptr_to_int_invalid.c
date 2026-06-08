// Test: pointer-to-integer cast is forbidden
// Expected: compile error "Casting pointer to integer"

#include <stdio.h>

int main(void) {
    int x = 42;
    int *p = &x;
    // This should be rejected: losing bounds metadata
    int addr = (int)p;
    printf("%d\n", addr);
    return 0;
}
