/* Phase 5: string::fn calls work without use namespace */
include string

main() {
    auto s[20];
    s = "hello";
    putnumbs(string::strlen(s));
}
