/* Phase 5: double include of string is silently skipped by the guard */
include string
include string
use namespace string

main() {
    auto s;
    s = "hi";
    putnumbs(strlen(s));   /* 2 */
}
