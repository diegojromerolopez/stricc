int* get_stack_ptr() {
    int x = 42;
    return &x; // Should be rejected at compile-time
}

int main() {
    int *p = get_stack_ptr();
    return 0;
}
