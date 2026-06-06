int main() {
    int x = 2147483647;
    int y = x + 1; // Should dynamically abort
    return 0;
}
