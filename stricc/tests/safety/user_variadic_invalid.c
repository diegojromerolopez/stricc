// Test: user-defined variadic functions are forbidden
// Expected: compile error "User-defined variadic function"

#include <stdio.h>

void my_printf(const char *fmt, ...) {
    // Attempting to define a variadic function body
    printf("bad\n");
}

int main(void) {
    my_printf("test %d\n", 42);
    return 0;
}
