void* malloc(unsigned long size);
void free(void* ptr);

int main() {
    int *ptr = (int*)malloc(4);
    *ptr = 42;
    free(ptr);
    int val = *ptr; // Should dynamically abort
    return 0;
}
