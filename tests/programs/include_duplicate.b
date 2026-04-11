/* Duplicate include — last registration wins, behaviour is identical.
   Expected output: 3
*/
include math
include math

main() {
    putnumbs(sqrt(9));
}
