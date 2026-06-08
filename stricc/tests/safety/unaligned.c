int main() {
    char buf[8];
    buf[0] = 0;
    int *p = (int*)(buf + 1); // Unaligned pointer to int (needs 4-byte alignment)
    int val = *p; // Should abort: unaligned dereference
    return val;
}
