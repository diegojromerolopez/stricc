void *memcpy(void *dest, const void *src, unsigned long n);

int main() {
    char buf[8];
    buf[0] = 'a';
    buf[1] = 'b';
    buf[2] = 'c';
    buf[3] = 'd';
    
    // Copy overlapping memory: buf + 1 from buf (len 3)
    // dest: buf + 1, src: buf, len 3
    // Expected buf: a, a, b, c
    memcpy(buf + 1, buf, 3);
    
    if (buf[0] != 'a' || buf[1] != 'a' || buf[2] != 'b' || buf[3] != 'c') {
        return 1;
    }
    return 0;
}
