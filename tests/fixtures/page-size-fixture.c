/* Checks that depend on the host kernel's page size, for aarch64.
 *
 * x86-64 Linux has 4K pages. arm64 kernels are built for 4K, 16K, or 64K,
 * and a supervisor that assumes 4K is wrong on the other two in ways that
 * only show there. Run as an instance's init by
 * scripts/test-native-page-size.py, this prints one line for each thing
 * that has gone wrong that way:
 *
 *   getcwd-page           getcwd with a buffer of one page, which is what a
 *                         C library's getcwd(NULL, 0) asks for
 *   statm-matches-status  /proc/self/statm counts pages and
 *                         /proc/self/status reports the same size in kB
 *   second-task-clone3    clone3 from a task other than the first, whose
 *                         argument snapshot is mapped at the second slot
 *   second-task-send      a send from that task, likewise
 *
 * On a 4K-page kernel all four pass whatever the supervisor assumes. */
#if !defined(__aarch64__)
#error "page sizes other than 4K exist only on arm64; this fixture is for aarch64"
#endif

typedef unsigned long word;

#include "fixture-aarch64.h"

static void print_result(const char *label, long value)
{
    char line[96];
    char digits[24];
    word length = 0, at = sizeof digits;
    word magnitude = value < 0 ? -(word)value : (word)value;
    do digits[--at] = '0' + magnitude % 10; while (magnitude /= 10);
    if (value < 0) digits[--at] = '-';
    while (*label && length < sizeof line - 26) line[length++] = *label++;
    while (at < sizeof digits) line[length++] = digits[at++];
    line[length++] = '\n';
    call3(1, 1, (word)line, length);
}

static char text[4096];

/* The file's contents, null-terminated, or an empty string. */
static const char *contents(const char *path)
{
    long fd = call3(2, (word)path, 0, 0);
    long length = fd < 0 ? 0 : call3(0, fd, (word)text, sizeof text - 1);
    if (fd >= 0) call3(3, fd, 0, 0);
    text[length < 0 ? 0 : length] = 0;
    return text;
}

/* The decimal number at `at`, after any spaces and tabs. */
static long number(const char *at)
{
    long value = 0;
    while (*at == ' ' || *at == '\t') ++at;
    if (*at < '0' || *at > '9') return -1;
    while (*at >= '0' && *at <= '9') value = value * 10 + (*at++ - '0');
    return value;
}

/* Where the text after `key` starts in `in`, or an empty string. */
static const char *after(const char *in, const char *key)
{
    for (; *in; ++in) {
        const char *left = in, *right = key;
        while (*right && *left == *right) ++left, ++right;
        if (!*right) return left;
    }
    return in;
}

long fixture_main(const word *stack)
{
    static char directory[65536];
    word page = fixture_page_size(stack);
    print_result("GINUX_FIXTURE: page-size=", (long)page);

    print_result("GINUX_FIXTURE: getcwd-page=", call3(79, (word)directory, page, 0) > 0);

    long pages = number(contents("/proc/self/statm"));
    long kilobytes = number(after(contents("/proc/self/status"), "VmSize:"));
    print_result("GINUX_FIXTURE: statm-matches-status=",
                 pages > 0 && pages * (long)page == kilobytes * 1024);

    int status = 0;
    long child = call3(57, 0, 0, 0);
    if (child == 0) {
        /* The first eight fields: version 0 of clone_args. */
        struct {
            word flags, pidfd, child_tid, parent_tid, exit_signal, stack, stack_size, tls;
        } args = {0};
        int inner = 0;
        args.exit_signal = 17 /* SIGCHLD */;
        long grandchild = call3(435, (word)&args, sizeof args, 0);
        if (grandchild == 0) call3(231, 7, 0, 0);
        int cloned = grandchild > 0
            && call4(61, grandchild, (word)&inner, 0, 0) == grandchild
            && ((inner >> 8) & 0xff) == 7;

        static struct { unsigned short family; char path[108]; } destination
            = {1 /* AF_UNIX */, "/tmp/page-size.sock"};
        static char payload = 'p';
        long sender = call3(41, 1 /* AF_UNIX */, 2 /* SOCK_DGRAM */, 0);
        long sent = sender < 0 ? sender
            : call6(44, sender, (word)&payload, 1, 0, (word)&destination, sizeof destination);
        call3(231, (cloned ? 1 : 0) | (sent == 1 ? 2 : 0), 0, 0);
    }
    long reaped = child > 0 ? call4(61, child, (word)&status, 0, 0) : -1;
    int code = reaped == child ? (status >> 8) & 0xff : 0;
    print_result("GINUX_FIXTURE: second-task-clone3=", code & 1);
    print_result("GINUX_FIXTURE: second-task-send=", (code >> 1) & 1);
    return 0;
}

__asm__(".global _start\n"
        "_start:\n"
        "mov x29, #0\n"
        "mov x30, #0\n"
        "mov x0, sp\n"
        "bl fixture_main\n"
        "mov x8, #94\n"
        "svc #0\n");
