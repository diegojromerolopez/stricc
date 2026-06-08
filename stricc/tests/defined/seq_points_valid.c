int main() {
    int i = 0;
    int a = (i = 5);
    int b = i + 1;
    int c = i++ && i++;
    int d = i++ || i++;
    return 0;
}
