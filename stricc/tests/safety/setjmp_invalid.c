// Test: setjmp is forbidden in Safe C mode
// Expected: compile error "setjmp/longjmp are forbidden"
#include <stdio.h>

int main(void) {
    setjmp(0);
    return 0;
}
