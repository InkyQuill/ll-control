/* Private MockBackend storage acceptance. Never preload into a production daemon. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

static int fault(const char *path, const char *operation) {
    const char *root = getenv("LLCONTROL_SMOKE_ROOT");
    char marker[4096], rule[128], expected[4096];
    if (!root || root[0] != '/' || !path) return 0;
    snprintf(marker, sizeof marker, "%s/fixture-marker", root);
    if (access(marker, F_OK)) return 0;
    snprintf(marker, sizeof marker, "%s/storage-fault", root);
    int fd = syscall(SYS_openat, AT_FDCWD, marker, O_RDONLY, 0);
    if (fd < 0) return 0;
    ssize_t n = syscall(SYS_read, fd, rule, sizeof rule - 1);
    syscall(SYS_close, fd);
    if (n <= 0) return 0;
    rule[n] = 0;
    char selector[80], error[20];
    if (sscanf(rule, "%79s %19s", selector, error) != 2) return 0;
    const char *suffix = NULL;
    int prefix = 0;
    if (!strcmp(selector, "settings-file-sync") && !strcmp(operation, "fsync")) {
        suffix = "/settings/devices/"; prefix = 1;
    } else if (!strcmp(selector, "settings-directory-sync") && !strcmp(operation, "fsync")) {
        suffix = "/settings/devices";
    } else if (!strcmp(selector, "settings-rename") && !strcmp(operation, "rename")) {
        suffix = "/settings/devices/"; prefix = 1;
    } else if (!strcmp(selector, "settings-write") && !strcmp(operation, "write")) {
        suffix = "/settings/devices/"; prefix = 1;
    } else if (!strcmp(selector, "media-file-sync") && !strcmp(operation, "fsync")) {
        suffix = "/settings/prepared-clips/.llcontrol-media-publish-"; prefix = 1;
    } else if (!strcmp(selector, "media-directory-sync") && !strcmp(operation, "fsync")) {
        suffix = "/settings/prepared-clips";
    }
    if ((!strcmp(selector, "journal-stage-sync") || !strcmp(selector, "profiles-stage-sync"))
        && !strcmp(operation, "fsync")) {
        snprintf(marker, sizeof marker, "%s/settings/screen-reset-transaction.json", root);
        int published = access(marker, F_OK) == 0;
        if ((!strcmp(selector, "journal-stage-sync") && !published)
            || (!strcmp(selector, "profiles-stage-sync") && published)) {
            suffix = "/settings/.tmp"; prefix = 1;
        }
    }
    if (!suffix) return 0;
    snprintf(expected, sizeof expected, "%s%s", root, suffix);
    if (prefix ? strncmp(path, expected, strlen(expected)) : strcmp(path, expected)) return 0;
    int code = !strcmp(error, "ENOSPC") ? ENOSPC : EIO;
    snprintf(marker, sizeof marker, "%s/storage-fault-events.log", root);
    fd = syscall(SYS_openat, AT_FDCWD, marker, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) {
        char line[4400];
        int length = snprintf(line, sizeof line, "%s %s %s\n", operation, error, path);
        if (length > 0 && (size_t)length < sizeof line) syscall(SYS_write, fd, line, length);
        syscall(SYS_close, fd);
    }
    errno = code;
    return 1;
}
static int fd_fault(int fd, const char *operation) {
    char link[80], path[4096];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    ssize_t n = readlink(link, path, sizeof path - 1);
    if (n < 0) return 0;
    path[n] = 0;
    return fault(path, operation);
}
int fsync(int fd) {
    if (fd_fault(fd, "fsync")) return -1;
    return syscall(SYS_fsync, fd);
}
ssize_t write(int fd, const void *buffer, size_t size) {
    if (fd_fault(fd, "write")) return -1;
    return syscall(SYS_write, fd, buffer, size);
}
int rename(const char *old, const char *next) {
    if (fault(old, "rename")) return -1;
    return syscall(SYS_renameat, AT_FDCWD, old, AT_FDCWD, next);
}
