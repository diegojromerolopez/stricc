int printf(const char *format, ...);

void my_printf(const char *fmt, int x, const char *s, double d) {
    printf(fmt, x, s, d);
}

int main() {
    my_printf("%d %s %f\n", 42, "hello", 3.14);
    return 0;
}
