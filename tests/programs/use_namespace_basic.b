/* Phase 5: basic use namespace string — strlen with bare name */
include string
use namespace string

main() {
    auto s[20];
    s = "hello";
    putnumbs(strlen(s));
}
