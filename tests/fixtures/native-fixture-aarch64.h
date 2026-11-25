/* aarch64 half of native-fixture.c: the calls from fixture-aarch64.h, then
 * the helpers that are assembly on either architecture. */

#include "fixture-aarch64.h"

/* rt_sigreturn (139). */
void sigchld_restorer(void);
__asm__(".global sigchld_restorer\n"
        "sigchld_restorer:\n"
        "mov x8, #139\n"
        "svc #0\n");

/* clone (220) with the thread flags; the child stops at a breakpoint. */
long native_thread_trap(word flags, word stack);
__asm__(".global native_thread_trap\n"
        "native_thread_trap:\n"
        "mov x2, #0\n"
        "mov x3, #0\n"
        "mov x4, #0\n"
        "mov x8, #220\n"
        "svc #0\n"
        "cbnz x0, 1f\n"
        "brk #0\n"
        "udf #0\n"
        "1: ret\n");

/* clone3 (435); the child exits 7 through exit_group (94). */
long native_clone_spawn(void *args, word size);
__asm__(".global native_clone_spawn\n"
        "native_clone_spawn:\n"
        "mov x8, #435\n"
        "svc #0\n"
        "cbnz x0, 1f\n"
        "mov x0, #7\n"
        "mov x8, #94\n"
        "svc #0\n"
        "udf #0\n"
        "1: ret\n");

/* A CLONE_THREAD child that issues one aarch64 call with four arguments,
 * stores the result in *result and exits. The child shares the caller's
 * memory but not its stack, so everything it needs is in callee-saved
 * registers before the clone. */
long native_thread_svc(word flags, word stack, word number, word a, word b, word c, word d,
                       volatile long *result);
__asm__(".global native_thread_svc\n"
        "native_thread_svc:\n"
        "stp x19, x20, [sp, #-48]!\n"
        "stp x21, x22, [sp, #16]\n"
        "stp x23, x24, [sp, #32]\n"
        "mov x19, x2\n"
        "mov x20, x3\n"
        "mov x21, x4\n"
        "mov x22, x5\n"
        "mov x23, x6\n"
        "mov x24, x7\n"
        "mov x2, #0\n"
        "mov x3, #0\n"
        "mov x4, #0\n"
        "mov x8, #220\n"
        "svc #0\n"
        "cbnz x0, 1f\n"
        "mov x8, x19\n"
        "mov x0, x20\n"
        "mov x1, x21\n"
        "mov x2, x22\n"
        "mov x3, x23\n"
        "svc #0\n"
        "str x0, [x24]\n"
        "mov x8, #93\n"
        "mov x0, #0\n"
        "svc #0\n"
        "udf #0\n"
        "1: ldp x23, x24, [sp, #32]\n"
        "ldp x21, x22, [sp, #16]\n"
        "ldp x19, x20, [sp], #48\n"
        "ret\n");

/* A CLONE_THREAD child that stores the result of x86-64 syscall `number`
 * with arguments a and b in *result and exits. */
static long native_thread_call(word flags, word stack, word number, word a, word b,
                               volatile long *result)
{
    struct frame frame = {(long)number, {a, b, 0, 0, 0, 0}, {0, 0}};
    if (lower(&frame) < 0) return -38;
    return native_thread_svc(flags, stack, (word)frame.number, frame.reg[0], frame.reg[1],
                             frame.reg[2], frame.reg[3], result);
}

/* The three gated threads below read and write the fixture's own volatile
 * words by name; native-fixture.c defines them. fcntl is 25, dup is 23,
 * dup3 is 24 and exit is 93. */
long native_gated_fcntl(word flags, word stack, const void *lock);
__asm__(".global native_gated_fcntl\n"
        "native_gated_fcntl:\n"
        "str x19, [sp, #-16]!\n"
        "mov x19, x2\n"
        "mov x2, #0\n"
        "mov x3, #0\n"
        "mov x4, #0\n"
        "mov x8, #220\n"
        "svc #0\n"
        "cbnz x0, 2f\n"
        "adrp x9, descriptor_mutation_start\n"
        "add x9, x9, :lo12:descriptor_mutation_start\n"
        "1: ldr x10, [x9]\n"
        "cbz x10, 1b\n"
        "adrp x9, descriptor_mutation_entered\n"
        "add x9, x9, :lo12:descriptor_mutation_entered\n"
        "mov x10, #1\n"
        "str x10, [x9]\n"
        "mov x8, #25\n"
        "mov x0, #1\n"
        "mov x1, #7\n"
        "mov x2, x19\n"
        "svc #0\n"
        "adrp x9, descriptor_mutation_result\n"
        "add x9, x9, :lo12:descriptor_mutation_result\n"
        "str x0, [x9]\n"
        "mov x8, #93\n"
        "mov x0, #0\n"
        "svc #0\n"
        "udf #0\n"
        "2: ldr x19, [sp], #16\n"
        "ret\n");

long native_gated_dup(word flags, word stack, word source);
__asm__(".global native_gated_dup\n"
        "native_gated_dup:\n"
        "str x19, [sp, #-16]!\n"
        "mov x19, x2\n"
        "mov x2, #0\n"
        "mov x3, #0\n"
        "mov x4, #0\n"
        "mov x8, #220\n"
        "svc #0\n"
        "cbnz x0, 2f\n"
        "adrp x9, descriptor_sibling_start\n"
        "add x9, x9, :lo12:descriptor_sibling_start\n"
        "1: ldr x10, [x9]\n"
        "cbz x10, 1b\n"
        "adrp x9, descriptor_sibling_entered\n"
        "add x9, x9, :lo12:descriptor_sibling_entered\n"
        "mov x10, #1\n"
        "str x10, [x9]\n"
        "mov x8, #23\n"
        "mov x0, x19\n"
        "svc #0\n"
        "adrp x9, descriptor_sibling_result\n"
        "add x9, x9, :lo12:descriptor_sibling_result\n"
        "str x0, [x9]\n"
        "mov x8, #93\n"
        "mov x0, #0\n"
        "svc #0\n"
        "udf #0\n"
        "2: ldr x19, [sp], #16\n"
        "ret\n");

/* aarch64 has no dup2; the thread duplicates with dup3 and no flags. */
long native_gap_dup2(word flags, word stack, const volatile char *gate, word source,
                     word target);
__asm__(".global native_gap_dup2\n"
        "native_gap_dup2:\n"
        "stp x19, x20, [sp, #-32]!\n"
        "str x21, [sp, #16]\n"
        "mov x19, x2\n"
        "mov x20, x3\n"
        "mov x21, x4\n"
        "mov x2, #0\n"
        "mov x3, #0\n"
        "mov x4, #0\n"
        "mov x8, #220\n"
        "svc #0\n"
        "cbnz x0, 2f\n"
        "adrp x9, descriptor_gap_ready\n"
        "add x9, x9, :lo12:descriptor_gap_ready\n"
        "mov x10, #1\n"
        "str x10, [x9]\n"
        "1: yield\n"
        "ldrb w10, [x19]\n"
        "cbz w10, 1b\n"
        "mov x8, #24\n"
        "mov x0, x20\n"
        "mov x1, x21\n"
        "mov x2, #0\n"
        "svc #0\n"
        "adrp x9, descriptor_gap_result\n"
        "add x9, x9, :lo12:descriptor_gap_result\n"
        "str x0, [x9]\n"
        "mov x8, #93\n"
        "mov x0, #0\n"
        "svc #0\n"
        "udf #0\n"
        "2: ldr x21, [sp, #16]\n"
        "ldp x19, x20, [sp], #32\n"
        "ret\n");

/* exit_group (94) with the scenario's return value. */
__asm__(".global _start\n"
        "_start:\n"
        "mov x29, #0\n"
        "mov x30, #0\n"
        "mov x0, sp\n"
        "bl native_fixture_main\n"
        "mov x8, #94\n"
        "svc #0\n");
