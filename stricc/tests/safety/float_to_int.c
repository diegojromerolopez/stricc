int main() {
    double d = 1e20;
    int x = (int)d; // Out of range for int, should abort
    return x;
}
