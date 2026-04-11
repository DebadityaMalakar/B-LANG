/* Phase 5: case conversion — use getvec for writable destinations */
include string
use namespace string

main() {
    auto src, dst;
    src = "hello_world";
    dst = getvec(30);
    tocamel(src, dst);
    putstr(dst);           /* helloWorld */
    rlsvec(dst);
    putchar(' ');

    src = "helloWorld";
    dst = getvec(30);
    tosnake(src, dst);
    putstr(dst);           /* hello_world */
    rlsvec(dst);
    putchar(' ');

    src = "hello world";
    dst = getvec(30);
    totitle(src, dst);
    putstr(dst);           /* Hello World */
    rlsvec(dst);
}
