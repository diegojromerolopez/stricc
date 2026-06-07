int main() {
    int x = 1073741824;
    int y = x * 2; // Should overflow and dynamically abort
    return 0;
}
