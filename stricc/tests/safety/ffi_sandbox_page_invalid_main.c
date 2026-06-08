void page_overflow(int *p);

int main() {
    int x = 42;
    page_overflow(&x);
    return 0;
}
