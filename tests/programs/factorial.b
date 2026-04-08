fact(n) {
    if (n <= 1) {
        return 1;
    }
    return n * fact(n - 1);
}

main() {
    putnumbs(fact(5));
    return 0;
}
