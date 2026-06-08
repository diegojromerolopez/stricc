void* aligned_alloc(unsigned long alignment, unsigned long size);
void free(void* ptr);

int main() {
    void *ptr = aligned_alloc(64, 128);
    if (!ptr) {
        return 1;
    }

    // Verify alignment
    unsigned long addr = (unsigned long)ptr;
    if ((addr & (unsigned long)63) != 0) {
        return 2;
    }

    // Write and read
    int *int_ptr = (int *)ptr;
    int_ptr[0] = 42;
    int_ptr[31] = 99;

    if (int_ptr[0] != 42 || int_ptr[31] != 99) {
        return 3;
    }

    free(ptr);
    return 0;
}
