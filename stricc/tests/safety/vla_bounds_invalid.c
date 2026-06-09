// Test: VLA with out-of-bounds access should abort
// Expected: runtime abort "Out-of-bounds pointer access"

int main() {
    int n = 3;
    int arr[n];
    arr[0] = 10;
    arr[1] = 20;
    arr[2] = 30;
    // Out-of-bounds access
    arr[5] = 99;
    return 0;
}
