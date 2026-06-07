int main() {
    int x = 1;
    int y = x << 35; // Shift count 35 is out of bounds for type Int (32 bits)
    return 0;
}
