void recurse(int depth) {
    char dummy[1024];
    dummy[0] = (char)depth;
    recurse(depth + 1);
}

int main() {
    recurse(1);
    return 0;
}
