void* malloc(unsigned long size);
void free(void* ptr);

int main() {
    int *ptr = (int*)malloc(4);
    free(ptr);
    free(ptr); // Should dynamically abort
    return 0;
}
