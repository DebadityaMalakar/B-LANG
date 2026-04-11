/* Phase 4 fixed-point arithmetic.
   pi   = m_pi()         => Q16 pi  = 205887
   half = itofp(1) / 2   => Q16 0.5 = 32768
   fpmul(pi, half)        => Q16 pi/2 = 102943
   fptoi(102943)          => 1
   Expected output: 1
*/
include math

main() {
    auto pi, half;
    pi = m_pi();
    half = itofp(1) / 2;
    putnumbs(fptoi(fpmul(pi, half)));
}
