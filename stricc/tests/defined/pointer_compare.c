void *malloc(unsigned long size);
void free(void *ptr);

int main() {
    int x = 42;
    int y = 99;
    
    int *px = &x;
    int *py = &y;
    
    // px and py are different stack variables.
    // They must have a consistent total order, so (px < py) or (px > py) is true.
    // (px <= py) and (px >= py) must also be consistent.
    
    if (px == py) {
        return 1;
    }
    
    if (px < py) {
        if (!(px <= py)) return 2;
        if (px > py) return 3;
        if (px >= py) return 4;
    } else {
        if (!(px >= py)) return 5;
        if (px < py) return 6;
        if (px <= py) return 7;
    }
    
    return 0;
}
