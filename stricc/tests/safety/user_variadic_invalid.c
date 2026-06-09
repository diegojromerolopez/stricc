// Test: user-defined variadic functions are forbidden
// Expected: compile error "User-defined variadic function"

void my_printf(const char *fmt, ...) {
    // Attempting to define a variadic function body
}

int main() {
    my_printf("test %d\n", 42);
    return 0;
}
