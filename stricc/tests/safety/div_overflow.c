int main() {
    int x = -2147483648; // INT_MIN
    int y = -1;
    int z = x / y; // INT_MIN / -1 overflows: should abort
    return z;
}
