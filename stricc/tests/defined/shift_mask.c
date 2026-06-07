int main() {
    int val = 1;
    int shift_35 = 35;
    int shift_neg_5 = -5;
    
    int res1 = val << shift_35;
    if (res1 != 8) {
        return 1;
    }
    
    int val_8 = 8;
    int res2 = val_8 >> shift_35;
    if (res2 != 1) {
        return 2;
    }
    
    int res3 = val << shift_neg_5;
    if (res3 != 134217728) {
        return 3;
    }
    
    // Test char (8 bits, mask is 7)
    char c = 1;
    int shift_9 = 9; // 9 & 7 = 1
    char res4 = c << shift_9;
    if (res4 != 2) {
        return 4;
    }
    
    // Test long (64 bits, mask is 63)
    long l = 1;
    int shift_66 = 66; // 66 & 63 = 2
    long res5 = l << shift_66;
    if (res5 != 4) {
        return 5;
    }
    
    return 0;
}
