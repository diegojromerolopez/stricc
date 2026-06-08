void modify_buffer(int *p, int value);

int main() {
    int x = 42;
    modify_buffer(&x, 100);
    if (x != 100) {
        return 1;
    }
    return 0;
}
