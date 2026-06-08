void* aligned_alloc(unsigned long alignment, unsigned long size);

int main() {
    void *ptr = aligned_alloc(3, 10);
    return 0;
}
