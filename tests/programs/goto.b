main() {
    auto i;
    i = 0;
L:  i = i + 1;
    if (i < 3) {
        goto L;
    }
    putnumbs(i);
    return 0;
}
