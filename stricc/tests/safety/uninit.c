int main() {
    int x;
    return 10 / x; // x is default-initialized to 0, so this triggers "Division by zero" abort
}
