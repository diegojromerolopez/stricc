void overflow_buffer(int *p);

int main() {
    int x = 42;
    overflow_buffer(&x);
    return 0;
}
