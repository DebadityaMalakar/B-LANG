/* String operations: lchar, char, putstr, concat.
   Builds "hi" via lchar, reads it back via char and putchar,
   then concatenates two strings and prints the result.
   Expected output: "hihello world"
*/
main() {
    auto s[10], a[6], b[7], dst[20], c;

    /* Build "hi" manually. */
    lchar(s, 0, 'h');
    lchar(s, 1, 'i');
    lchar(s, 2, '\*e');

    /* Read back individual chars. */
    c = char(s, 0);
    putchar(c);
    c = char(s, 1);
    putchar(c);

    /* Build "hello" and " world" then concat into dst. */
    lchar(a, 0, 'h');
    lchar(a, 1, 'e');
    lchar(a, 2, 'l');
    lchar(a, 3, 'l');
    lchar(a, 4, 'o');
    lchar(a, 5, '\*e');

    lchar(b, 0, ' ');
    lchar(b, 1, 'w');
    lchar(b, 2, 'o');
    lchar(b, 3, 'r');
    lchar(b, 4, 'l');
    lchar(b, 5, 'd');
    lchar(b, 6, '\*e');

    concat(dst, a, b);
    putstr(dst);
}
