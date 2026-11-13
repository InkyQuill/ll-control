/* Linux native-smoke fixture only. Loaded into desktop-mock, never the GUI. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static int reject(const char *path) {
    const char *root = getenv("LLCONTROL_SMOKE_ROOT");
    char marker[4096], target[4096];
    if (!root || root[0] != '/') return 0;
    int n = snprintf(marker, sizeof marker, "%s/fail-unlink", root);
    if (n < 0 || (size_t)n >= sizeof marker || access(marker, F_OK)) return 0;
    const char *relative[] = {
        "settings/theme-assets/ffffffffffffffffffffffffffffffff.png",
        "settings/theme-fonts/ffffffffffffffffffffffffffffffff.font"
    };
    for (size_t i = 0; i < sizeof relative / sizeof relative[0]; i++) {
        n = snprintf(target, sizeof target, "%s/%s", root, relative[i]);
        if (n > 0 && (size_t)n < sizeof target && !strcmp(path, target)) {
            errno = EACCES;
            return 1;
        }
    }
    return 0;
}
int unlink(const char *path) {
    if (reject(path)) return -1;
    return syscall(SYS_unlinkat, AT_FDCWD, path, 0);
}
int unlinkat(int dirfd, const char *path, int flags) {
    if (reject(path)) return -1;
    return syscall(SYS_unlinkat, dirfd, path, flags);
}
