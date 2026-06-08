int main() {
    int *p;
    {
        int x = 42;
        p = &x;
    }
    // Accessing *p should abort due to stack use-after-free
    int val = *p;
    return 0;
}
