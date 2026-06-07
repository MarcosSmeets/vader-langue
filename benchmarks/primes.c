#include <stdio.h>
static int is_prime(long n) {
    if (n < 2) return 0;
    for (long i = 2; i * i <= n; i++)
        if (n % i == 0) return 0;
    return 1;
}
int main(void) {
    long count = 0;
    for (long n = 2; n < 2000000; n++)
        if (is_prime(n)) count++;
    printf("%ld\n", count);
    return 0;
}
