/* Phase 4 trig — sin(pi/2) should be Q16 1.0 = 65536.
   1571 milliradians is the nearest integer to pi/2 (1570.796... mr).
   Expected output: 65536
*/
include math

main() {
    putnumbs(sin(1571));
}
