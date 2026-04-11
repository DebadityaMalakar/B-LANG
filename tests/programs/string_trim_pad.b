/* Phase 5: trim and pad */
include string
use namespace string

main() {
    auto s, dst, padded;
    s = "  hello  ";
    dst = getvec(30);
    strip(s, dst);
    putstr(dst);           /* hello */
    rlsvec(dst);
    putchar(' ');

    padded = lpad("42", 5, '0');
    putstr(padded);        /* 00042 */
    rlsvec(padded);
}
