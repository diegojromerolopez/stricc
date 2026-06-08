int main() {
    int x = 42;
    int *p = &x;
    int a = *p;
    int b = *p;
    if (a != 42 || b != 42) {
        return 1;
    }
    return 0;
}
