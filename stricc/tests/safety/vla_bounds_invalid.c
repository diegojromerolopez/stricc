// Test: VLA with out-of-bounds access should abort
// Expected: runtime abort "Out-of-bounds pointer access"

#include <stdio.h>

int main(void) {
    int n = 3;
    int arr[n];
    arr[0] = 10;
    arr[1] = 20;
    arr[2] = 30;
    // Out-of-bounds access
    arr[5] = 99;
    printf("%d\n", arr[5]);
    return 0;
}
