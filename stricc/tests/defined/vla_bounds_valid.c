// Test: VLA within bounds — should compile and run cleanly
// Expected: clean exit

#include <stdio.h>

int main(void) {
    int n = 5;
    int arr[n];
    for (int i = 0; i < n; i++) {
        arr[i] = i * 2;
    }
    int expected = 0 + 2 + 4 + 6 + 8; // 20
    int actual = 0;
    for (int i = 0; i < n; i++) {
        actual += arr[i];
    }
    if (actual != expected) {
        return 1;
    }
    return 0;
}
