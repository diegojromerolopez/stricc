struct Point {
    int x;
    int y;
};

int main() {
    int x;
    double d;
    int* p;
    struct Point pt;
    
    if (x != 0) {
        return 1;
    }
    if (d != 0.0) {
        return 2;
    }
    if (p != nullptr) {
        return 3;
    }
    if (pt.x != 0) {
        return 4;
    }
    if (pt.y != 0) {
        return 5;
    }
    
    return 0;
}
