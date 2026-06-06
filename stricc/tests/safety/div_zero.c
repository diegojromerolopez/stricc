int main() {
    int x = 42;
    int y = 0;
    int z = x / y; // Should dynamically abort
    return 0;
}
