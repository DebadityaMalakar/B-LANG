/* Phase 5: number conversion */
include string
use namespace string

main() {
    auto buf;
    buf = getvec(20);

    itoa(255, buf);
    putstr(buf);           /* 255  */
    putchar(' ');

    itoax(255, buf);
    putstr(buf);           /* ff   */
    putchar(' ');

    itoao(8, buf);
    putstr(buf);           /* 10   */
    putchar(' ');

    putnumbs(atoi("123abc"));  /* 123  */
    rlsvec(buf);
}
