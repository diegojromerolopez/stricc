// Test: setjmp is forbidden in Safe C mode
// Expected: compile error "setjmp/longjmp are forbidden"

int main() {
    setjmp(0);
    return 0;
}
