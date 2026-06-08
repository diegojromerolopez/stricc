// Test: for loop with a simple counter — verifies For barrier is emitted correctly
// Expected: clean exit

#include <stdio.h>

int main(void) {
    int sum = 0;
    for (int i = 0; i < 10; i++) {
        sum += i;
    }
    // 0+1+2+...+9 = 45
    if (sum != 45) {
        return 1;
    }
    return 0;
}
