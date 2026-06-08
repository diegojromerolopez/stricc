int main() {
    const int x = 42;
    const int *p = &x;
    int *q = (int*)p;
    return 0;
}
