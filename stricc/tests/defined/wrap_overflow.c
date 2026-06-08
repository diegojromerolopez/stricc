int main() {
    unsigned int x = 2147483647;
    unsigned int y = x + 1;
    if (y != 2147483648) {
        return 1;
    }
    return 0;
}
