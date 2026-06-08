// vader_test.c — native `vader test` harness. Linked only in test builds.
//
// The generated `main` registers every user function for coverage, then runs each
// `test` block through vader_test_run. A failing assert/panic longjmps back here
// (see g_in_test/g_test_jmp in vader_rt.c), so it fails only that test. Coverage
// persists across tests because everything runs in one process (no fork).

#include <setjmp.h>
#include <stdio.h>
#include <string.h>

extern int g_in_test;       // defined in vader_rt.c
extern jmp_buf g_test_jmp;  // defined in vader_rt.c

#define VADER_COV_MAX 8192

static const char *g_all[VADER_COV_MAX];
static int g_nall = 0;
static const char *g_hit[VADER_COV_MAX];
static int g_nhit = 0;
static int g_passed = 0;
static int g_failed = 0;

// Registers a function in the coverage denominator (called once per function).
void vader_cov_register(const char *fn) {
    for (int i = 0; i < g_nall; i++)
        if (strcmp(g_all[i], fn) == 0)
            return;
    if (g_nall < VADER_COV_MAX)
        g_all[g_nall++] = fn;
}

// Marks a function as covered (injected at each function's entry).
void vader_cov(const char *fn) {
    for (int i = 0; i < g_nhit; i++)
        if (strcmp(g_hit[i], fn) == 0)
            return;
    if (g_nhit < VADER_COV_MAX)
        g_hit[g_nhit++] = fn;
}

// Runs one test function; returns 1 if it passed, 0 if an assert/panic fired.
int vader_test_run(void (*fn)(void)) {
    g_in_test = 1;
    int failed = setjmp(g_test_jmp);
    if (!failed)
        fn();
    g_in_test = 0;
    return !failed;
}

// Prints the per-test result line and tallies it.
void vader_test_report_one(const char *name, int passed) {
    if (passed) {
        printf("  ✓ %s\n", name);
        g_passed++;
    } else {
        printf("  ✗ %s\n", name);
        g_failed++;
    }
}

// Prints the summary + coverage and returns the process exit code:
// 0 = all passed (and coverage ok), 1 = a test failed, 2 = coverage below the gate.
int vader_test_summary(int gate, double min_cov) {
    printf("\n%d passed, %d failed\n", g_passed, g_failed);
    double pct = g_nall ? (100.0 * g_nhit / g_nall) : 100.0;
    printf("coverage: %.1f%% (%d/%d functions)\n", pct, g_nhit, g_nall);
    if (g_failed > 0)
        return 1;
    if (gate && pct < min_cov) {
        printf("✗ coverage %.1f%% is below the minimum of %.1f%%\n", pct, min_cov);
        return 2;
    }
    return 0;
}
