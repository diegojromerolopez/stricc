void* malloc(unsigned long size);

int main() {
    int *ptr = (int*)malloc(5 * 4); // Allocates 20 bytes
    int *out_of_bounds = ptr + 6;   // Address offset by 24 bytes (out of bounds)
    *out_of_bounds = 42;            // Should dynamically abort
    return 0;
}
