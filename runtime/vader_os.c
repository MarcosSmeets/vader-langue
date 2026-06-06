/* Vader runtime std/os and std/env: access to the process environment. */
#include <stdlib.h>

extern char *vader_strdup(const char *s);

/* env.read(name): value of the environment variable, or "" if it doesn't exist. */
const char *vader_env_read(const char *name) {
    const char *v = getenv(name);
    return vader_strdup(v ? v : "");
}
