int get_val(int x) {
    if (x > 10) {
        return 1;
    }
    // Missing return: should be rejected at compile-time
}

int main() {
    int v = get_val(5);
    return 0;
}
