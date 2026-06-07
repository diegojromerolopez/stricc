int main() {
    int *ptr = nullptr;
    *ptr = 42; // Should dynamically abort (Out-of-bounds pointer access)
    return 0;
}
