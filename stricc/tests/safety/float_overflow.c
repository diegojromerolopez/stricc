int main() {
    double f = 1e300;
    double f2 = f * f; // Should overflow to infinity and trigger abort
    return 0;
}
