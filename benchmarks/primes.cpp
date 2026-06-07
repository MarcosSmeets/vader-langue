#include <cstdio>
static bool is_prime(long n) {
    if (n < 2) return false;
    for (long i = 2; i * i <= n; i++)
        if (n % i == 0) return false;
    return true;
}
int main() {
    long count = 0;
    for (long n = 2; n < 2000000; n++)
        if (is_prime(n)) count++;
    std::printf("%ld\n", count);
}
