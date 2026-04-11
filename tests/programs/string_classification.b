/* Phase 5: character classification */
include string
use namespace string

main() {
    putnumbs(isalpha('a'));     /* 1 */
    putchar(' ');
    putnumbs(isalpha('1'));     /* 0 */
    putchar(' ');
    putnumbs(isdigit('5'));     /* 1 */
    putchar(' ');
    putnumbs(isspace(' '));     /* 1 */
    putchar(' ');
    putnumbs(isupper('A'));     /* 1 */
    putchar(' ');
    putnumbs(islower('z'));     /* 1 */
}
