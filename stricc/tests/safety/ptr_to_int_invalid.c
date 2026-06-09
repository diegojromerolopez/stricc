// Test: pointer-to-integer cast is forbidden
// Expected: compile error "Casting pointer to integer"

int main() {
    int x = 42;
    int *p = &x;
    // This should be rejected: losing bounds metadata
    int addr = (int)p;
    return 0;
}
