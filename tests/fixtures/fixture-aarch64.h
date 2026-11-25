/* System calls for the C fixtures on aarch64.
 *
 * The scenarios name calls by their x86-64 numbers, as the supervisor's
 * contract does. On aarch64 each call is issued the way a C library for
 * that architecture issues it: the number comes from the generic table,
 * and a call the generic table lacks becomes its modern form (open is
 * openat, pipe is pipe2, dup2 is dup3, fork is clone, poll is ppoll, mkdir
 * is mkdirat). Argument encodings that differ between the two ABIs are
 * converted too, so a scenario passes x86-64 values on both.
 *
 * A fixture defines `word` and includes this in place of its x86-64 calls.
 * It provides call3 to call6, call_kept, spin_wait, breakpoint, and the
 * memset and memcpy gcc may call. */

/* Generated from crates/ginux-core/src/syscall_map.rs: the aarch64 number
 * plus one for each x86-64 number, zero where there is none. */
#include "syscall-map-aarch64.h"

#define spin_wait() __asm__ volatile("yield")
#define breakpoint() __asm__ volatile("brk #0")

struct frame {
    long number;
    word reg[6];
    word timeout[2];
};

/* One `svc #0` with frame->number in x8. The argument registers are read
 * back, so a caller can see what the kernel or a tracer left in x1 to x5. */
static long svc(struct frame *frame)
{
    register long x8 __asm__("x8") = frame->number;
    register word x0 __asm__("x0") = frame->reg[0];
    register word x1 __asm__("x1") = frame->reg[1];
    register word x2 __asm__("x2") = frame->reg[2];
    register word x3 __asm__("x3") = frame->reg[3];
    register word x4 __asm__("x4") = frame->reg[4];
    register word x5 __asm__("x5") = frame->reg[5];
    __asm__ volatile("svc #0"
                     : "+r"(x0), "+r"(x1), "+r"(x2), "+r"(x3), "+r"(x4), "+r"(x5)
                     : "r"(x8)
                     : "memory");
    frame->reg[1] = x1;
    frame->reg[2] = x2;
    frame->reg[3] = x3;
    frame->reg[4] = x4;
    frame->reg[5] = x5;
    return (long)x0;
}

/* O_DIRECT and O_LARGEFILE sit where aarch64 has O_DIRECTORY and
 * O_NOFOLLOW, and the other way round. The exchange is its own inverse. */
static word exchange_open_flags(word flags)
{
    return (flags & ~(word)0x3c000) | ((flags & 0xc000) << 2) | ((flags & 0x30000) >> 2);
}

static void shift_arguments(struct frame *frame, word first)
{
    for (int at = 5; at > 0; --at) frame->reg[at] = frame->reg[at - 1];
    frame->reg[0] = first;
}

/* Rewrite an x86-64 call in place as the aarch64 call. Returns how many
 * registers the caller's arguments moved up, or -1 when the call has no
 * aarch64 form. */
static int lower(struct frame *frame)
{
    int moved = 0;
    switch (frame->number) {
    case 2: /* open(path, flags, mode) */
        shift_arguments(frame, (word)-100 /* AT_FDCWD */);
        frame->number = 257;
        moved = 1;
        break;
    case 7: /* poll(descriptors, count, milliseconds) */
        frame->timeout[0] = frame->reg[2] / 1000;
        frame->timeout[1] = frame->reg[2] % 1000 * 1000000;
        frame->reg[2] = (long)frame->reg[2] < 0 ? 0 : (word)frame->timeout;
        frame->reg[3] = 0;
        frame->reg[4] = 0;
        frame->number = 271;
        break;
    case 22: /* pipe(descriptors) */
        frame->reg[1] = 0;
        frame->number = 293;
        break;
    case 33: /* dup2(source, target), for distinct descriptors */
        frame->reg[2] = 0;
        frame->number = 292;
        break;
    case 57: /* fork() */
        frame->reg[0] = 17 /* SIGCHLD */;
        frame->reg[1] = frame->reg[2] = frame->reg[3] = frame->reg[4] = 0;
        frame->number = 56;
        break;
    case 83: /* mkdir(path, mode) */
        shift_arguments(frame, (word)-100 /* AT_FDCWD */);
        frame->number = 258;
        moved = 1;
        break;
    default:
        break;
    }
    switch (frame->number) {
    case 56: { /* clone: x86-64 passes child_tid before tls */
        word child_tid = frame->reg[3];
        frame->reg[3] = frame->reg[4];
        frame->reg[4] = child_tid;
        break;
    }
    case 72: /* fcntl */
        if ((int)frame->reg[1] == 4 /* F_SETFL */)
            frame->reg[2] = exchange_open_flags(frame->reg[2]);
        break;
    case 257: /* openat */
        frame->reg[2] = exchange_open_flags(frame->reg[2]);
        break;
    case 293: /* pipe2 */
        frame->reg[1] = exchange_open_flags(frame->reg[1]);
        break;
    default:
        break;
    }
    if (frame->number < 0
        || (word)frame->number >= sizeof aarch64_number / sizeof aarch64_number[0]
        || aarch64_number[frame->number] == 0)
        return -1;
    frame->number = aarch64_number[frame->number] - 1;
    return moved;
}

static long call6(long number, word a, word b, word c, word d, word e, word f)
{
    struct frame frame = {number, {a, b, c, d, e, f}, {0, 0}};
    if (number == 33 && (unsigned)a == (unsigned)b) {
        /* dup2 onto itself returns the descriptor when it is open. dup3
         * rejects equal descriptors, so a C library asks fcntl instead. */
        long open = call6(72, a, 1 /* F_GETFD */, 0, 0, 0, 0);
        return open < 0 ? open : (long)(unsigned)b;
    }
    int status_flags = number == 72 && (int)b == 3 /* F_GETFL */;
    if (lower(&frame) < 0) return -38;
    long result = svc(&frame);
    if (status_flags && result >= 0) result = (long)exchange_open_flags((word)result);
    return result;
}

__attribute__((unused)) static long call3(long number, word a, word b, word c)
{
    return call6(number, a, b, c, 0, 0, 0);
}

__attribute__((unused)) static long call4(long number, word a, word b, word c, word d)
{
    return call6(number, a, b, c, d, 0, 0);
}

__attribute__((unused)) static long call5(long number, word a, word b, word c, word d, word e)
{
    return call6(number, a, b, c, d, e, 0);
}

/* Issue a call and report whether the registers that carried its first
 * three arguments still hold them when it returns. On aarch64 the result
 * comes back in x0, so an argument passed in x0 cannot be observed; every
 * other one is compared and replaced by what the register holds if it
 * changed. */
__attribute__((unused)) static long call_kept(long number, word *a, word *b, word *c, word d, word e)
{
    struct frame frame = {number, {*a, *b, *c, d, e, 0}, {0, 0}};
    word *const kept[3] = {a, b, c};
    int moved = lower(&frame);
    if (moved < 0) return -38;
    struct frame sent = frame;
    long result = svc(&frame);
    for (int at = 0; at < 3; ++at) {
        int reg = at + moved;
        if (reg != 0 && frame.reg[reg] != sent.reg[reg]) *kept[at] = frame.reg[reg];
    }
    return result;
}

/* gcc expands large aggregate initializers and copies into these. */
__attribute__((used, optimize("no-tree-loop-distribute-patterns")))
void *memset(void *target, int value, word count)
{
    volatile unsigned char *byte = target;
    while (count--) *byte++ = (unsigned char)value;
    return target;
}

__attribute__((used, optimize("no-tree-loop-distribute-patterns")))
void *memcpy(void *target, const void *source, word count)
{
    volatile unsigned char *to = target;
    const unsigned char *from = source;
    while (count--) *to++ = *from++;
    return target;
}
