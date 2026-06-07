/* Vader runtime std/math: floating-point + integer math (libm). */
#include <math.h>
#include <stdlib.h>
#include <time.h>

static int seeded = 0;
static void ensure_seed(void) {
    if (!seeded) { srand((unsigned)time(0)); seeded = 1; }
}

double vader_math_sqrt(double x) { return sqrt(x); }
double vader_math_pow(double b, double e) { return pow(b, e); }
double vader_math_abs(double x) { return fabs(x); }
double vader_math_floor(double x) { return floor(x); }
double vader_math_ceil(double x) { return ceil(x); }
double vader_math_round(double x) { return round(x); }
double vader_math_sin(double x) { return sin(x); }
double vader_math_cos(double x) { return cos(x); }
double vader_math_tan(double x) { return tan(x); }
double vader_math_log(double x) { return log(x); }
double vader_math_exp(double x) { return exp(x); }
double vader_math_fmin(double a, double b) { return a < b ? a : b; }
double vader_math_fmax(double a, double b) { return a > b ? a : b; }
double vader_math_pi(void) { return 3.14159265358979323846; }
long long vader_math_abs_int(long long n) { return n < 0 ? -n : n; }
long long vader_math_min_int(long long a, long long b) { return a < b ? a : b; }
long long vader_math_max_int(long long a, long long b) { return a > b ? a : b; }
double vader_math_random(void) { ensure_seed(); return (double)rand() / ((double)RAND_MAX + 1.0); }
long long vader_math_random_int(long long n) {
    ensure_seed();
    if (n <= 0) return 0;
    return (long long)(rand() % n);
}
