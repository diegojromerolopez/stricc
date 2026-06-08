union U {
    int i;
    float f;
};

int main() {
    union U u;
    u.i = 1065353216;
    if ((double)u.f != 1.0) {
        return 1;
    }
    return 0;
}
