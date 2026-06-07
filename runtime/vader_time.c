/* Vader runtime std/time: clocks, sleep, formatting, calendar fields. */
#include <time.h>
#include <stdio.h>

extern void *vader_alloc(long n);

long long vader_time_now(void) { return (long long)time(0); }
long long vader_time_now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}
void vader_time_sleep(long long ms) {
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, 0);
}
/* "YYYY-MM-DD HH:MM:SS" in local time */
char *vader_time_format(long long ts) {
    time_t t = (time_t)ts;
    struct tm tm;
    localtime_r(&t, &tm);
    char *o = vader_alloc(32);
    strftime(o, 32, "%Y-%m-%d %H:%M:%S", &tm);
    return o;
}
static struct tm tm_of(long long ts) {
    time_t t = (time_t)ts;
    struct tm tm;
    localtime_r(&t, &tm);
    return tm;
}
long long vader_time_year(long long ts) { return tm_of(ts).tm_year + 1900; }
long long vader_time_month(long long ts) { return tm_of(ts).tm_mon + 1; }
long long vader_time_day(long long ts) { return tm_of(ts).tm_mday; }
long long vader_time_hour(long long ts) { return tm_of(ts).tm_hour; }
long long vader_time_minute(long long ts) { return tm_of(ts).tm_min; }
long long vader_time_second(long long ts) { return tm_of(ts).tm_sec; }
