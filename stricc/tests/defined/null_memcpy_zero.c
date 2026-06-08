void *memcpy(void *dest, const void *src, unsigned long n);

int main() {
    // nullptr memcpy with size 0 should not abort
    memcpy(nullptr, nullptr, 0);
    return 0;
}
