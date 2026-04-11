/* Phase 4 RNG — deterministic with seed 42.
   Two calls with the same seed must produce the same sequence.
   The test runner verifies determinism by comparing two runs.
*/
include math

main() {
    srand(42);
    putnumbs(randrange(0, 100));
    putchar(' ');
    putnumbs(randrange(0, 100));
}
