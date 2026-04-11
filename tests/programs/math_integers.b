/* Phase 4 integer math builtins.
   Expected output: 42 6 12 1024 12 0
*/
include math

main() {
    putnumbs(abs(-42));
    putchar(' ');
    putnumbs(gcd(48, 18));
    putchar(' ');
    putnumbs(lcm(4, 6));
    putchar(' ');
    putnumbs(pow(2, 10));
    putchar(' ');
    putnumbs(sqrt(144));
    putchar(' ');
    putnumbs(clamp(-5, 0, 10));
}
