int main() {
    int x = -2147483647;
    int y = x - 2; // Should overflow (underflow) and dynamically abort
    return 0;
}
