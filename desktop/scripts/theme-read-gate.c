/* Linux fixture only: delay a single private PNG read, with a bounded timeout. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

ssize_t read(int fd, void *buf, size_t size) {
    const char *root = getenv("LCONNECT_SMOKE_ROOT");
    char proc[64], path[4096], target[4096], hold[4096], entered[4096];
    if (!root || root[0] != '/') return syscall(SYS_read, fd, buf, size);
    int n = snprintf(target, sizeof target, "%s/settings/theme-assets/22222222222222222222222222222222.png", root);
    if (n < 0 || (size_t)n >= sizeof target) return syscall(SYS_read, fd, buf, size);
    snprintf(proc, sizeof proc, "/proc/self/fd/%d", fd);
    ssize_t len = readlink(proc, path, sizeof path - 1);
    if (len < 0) return syscall(SYS_read, fd, buf, size);
    path[len] = '\0';
    if (strcmp(path, target)) return syscall(SYS_read, fd, buf, size);
    n = snprintf(hold, sizeof hold, "%s/hold-read", root);
    if (n < 0 || (size_t)n >= sizeof hold || access(hold, F_OK)) return syscall(SYS_read, fd, buf, size);
    n = snprintf(entered, sizeof entered, "%s/read-entered", root);
    if (n < 0 || (size_t)n >= sizeof entered) { errno = ENAMETOOLONG; return -1; }
    int marker = syscall(SYS_openat, AT_FDCWD, entered, O_CREAT | O_WRONLY | O_CLOEXEC, 0600);
    if (marker < 0) return -1;
    syscall(SYS_close, marker);
    for (int i = 0; i < 100; i++) {
        if (access(hold, F_OK)) return syscall(SYS_read, fd, buf, size);
        struct timespec pause = { .tv_sec = 0, .tv_nsec = 100000000 };
        nanosleep(&pause, NULL);
    }
    errno = ETIMEDOUT;
    return -1;
}
