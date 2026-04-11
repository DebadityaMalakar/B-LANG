/* Phase 5: string inspection functions — source strings are heap (literals) */
include string
use namespace string

main() {
    auto s[20];
    s = "hello";
    putnumbs(strlen(s));            /* 5  */
    putchar(' ');
    putnumbs(strcmp(s, "hello"));   /* 0  */
    putchar(' ');
    putnumbs(startswith(s, "hel")); /* 1  */
    putchar(' ');
    putnumbs(endswith(s, "lo"));    /* 1  */
    putchar(' ');
    putnumbs(indexof(s, "ll"));     /* 2  */
    putchar(' ');
    putnumbs(count(s, "l"));        /* 2  */
}
