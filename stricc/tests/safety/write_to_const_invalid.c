int main() {
    const int x = 42;
    const int *cp = &x;
    void *v = cp;
    int *p = v;
    *p = 100;
    return 0;
}
