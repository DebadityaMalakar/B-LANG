/* Nested while + switch with fallthrough.
   For i = 0: switch(0) -> case 0: print 0, break        -> "0"
   For i = 1: switch(1) -> case 1: print 1, fallthrough
                        -> case 2: print 2, break         -> "12"
   For i = 2: switch(2) -> case 2: print 2, break        -> "2"
   Expected output: "0122"
*/
main() {
    auto i;
    i = 0;
    while (i < 3) {
        switch (i) {
        case 0: putnumbs(0); break;
        case 1: putnumbs(1);
        case 2: { putnumbs(2); break; }
        default: { putnumbs(9); break; }
        }
        i = i + 1;
    }
}
