void overflow_buffer(int *p) {
    // Overwrite the memory beyond the 1-int buffer (index 1 of int is offset 4, which overlaps with the 8-byte canary)
    p[1] = 999;
}
