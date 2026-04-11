/* Edge-case math: domain errors return safe defaults, no crash.
   divf(7, 0) -> 0
   sqrt(-1)   -> 0
   Expected output: 0 0
*/
include math

main() {
    putnumbs(divf(7, 0));
    putchar(' ');
    putnumbs(sqrt(-1));
}
