/* Infinite recursion — should trigger StackOverflow. */
inf(n) {
    return(inf(n + 1));
}

main() {
    inf(0);
}
