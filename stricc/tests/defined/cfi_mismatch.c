void target(int x) {}

int main() {
    void *fp = (void*)target;
    fp(1.2);
    return 0;
}
