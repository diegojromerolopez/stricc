int printf(const char *format, ...);

void my_printf(const char *fmt, int x) {
    printf(fmt, x);
}

int main() {
    my_printf("%s\n", 42);
    return 0;
}
