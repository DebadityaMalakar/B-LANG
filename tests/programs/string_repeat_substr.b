/* Phase 5: repeat and substr */
include string
use namespace string

main() {
    auto s, dst, rep;
    s = "ab";
    rep = repeat(s, 3);
    putstr(rep);           /* ababab */
    rlsvec(rep);
    putchar(' ');

    s = "hello";
    dst = getvec(20);
    substr(s, 1, 3, dst);
    putstr(dst);           /* ell */
    rlsvec(dst);
}
