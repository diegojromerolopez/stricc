int main() {
    int x = 42;
    const int *p = &x;
    int y = *p;
    if (y != 42) {
        return 1;
    }
    return 0;
}
