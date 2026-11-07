.global _start
.section .text
_start:
    mov x0, #1
    adr x1, write_msg
    adr x2, write_msg_end
    sub x2, x2, x1
    mov x21, x2
    mov x8, #64
    svc #0
    cmp x0, x21
    b.ne fail

    mov x0, #0
    mov x8, #214
    svc #0
    cmp x0, #0
    b.le fail
    mov x19, x0
    add x0, x19, #4096
    mov x8, #214
    svc #0
    add x3, x19, #4096
    cmp x0, x3
    b.ne fail
    mov w1, #0x41
    strb w1, [x19]
    ldrb w2, [x19]
    cmp w2, w1
    b.ne fail

    mov x0, #0
    mov x1, #4096
    mov x2, #3
    mov x3, #0x22
    mov x4, #-1
    mov x5, #0
    mov x8, #222
    svc #0
    cmp x0, #0
    b.le fail
    mov w1, #0x42
    strb w1, [x0]
    ldrb w2, [x0]
    cmp w2, w1
    b.ne fail

    mov x0, #1
    adr x1, memory_msg
    adr x2, memory_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #0
    mov x1, #4096
    mov x2, #3
    mov x3, #0x22
    mov x4, #-1
    mov x5, #0
    mov x8, #222
    svc #0
    cmp x0, #0
    b.le fail
    add x22, x0, #4096

    mov x0, #17
    mov x1, x22
    mov x2, #0
    mov x3, #0
    mov x4, #0
    mov x8, #220
    svc #0
    cbz x0, clone_child
    cmp x0, #0
    b.le fail
    mov x24, x0
    mov x0, #1
    adr x1, clone_parent_msg
    adr x2, clone_parent_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #1
    mov x1, x24
    mov x2, x19
    mov x3, #4
    mov x4, #0
    mov x8, #95
    svc #0
    cmp x0, #0
    b.ne fail
    ldr w1, [x19]
    cmp w1, #17
    b.ne fail
    ldr w1, [x19, #8]
    cmp w1, #1
    b.ne fail
    ldr w1, [x19, #16]
    cmp w1, w24
    b.ne fail
    ldr w1, [x19, #24]
    cmp w1, #42
    b.ne fail
    mov x0, #1
    adr x1, wait_clone_msg
    adr x2, wait_clone_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    str xzr, [x19]
    str xzr, [x19, #8]
    str xzr, [x19, #16]
    str xzr, [x19, #24]
    mov x2, #17
    str x2, [x19, #32]
    sub x2, x22, #4096
    str x2, [x19, #40]
    mov x2, #4096
    str x2, [x19, #48]
    str xzr, [x19, #56]
    mov x0, x19
    mov x1, #64
    mov x8, #435
    svc #0
    cbz x0, clone3_child
    cmp x0, #0
    b.le fail
    mov x25, x0
    mov x0, #1
    adr x1, clone3_parent_msg
    adr x2, clone3_parent_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #1
    mov x1, x25
    mov x2, x19
    mov x3, #4
    mov x4, #0
    mov x8, #95
    svc #0
    cmp x0, #0
    b.ne fail
    ldr w1, [x19, #16]
    cmp w1, w25
    b.ne fail
    ldr w1, [x19, #24]
    cmp w1, #7
    b.ne fail
    mov x0, #1
    adr x1, wait_clone3_msg
    adr x2, wait_clone3_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #-100
    adr x1, disk_path
    mov x2, #2
    mov x3, #0
    mov x8, #56
    svc #0
    cmp x0, #3
    b.ne fail
    mov x23, x0

    mov x0, x23
    mov x1, x19
    mov x8, #80
    svc #0
    cmp x0, #0
    b.ne fail
    ldr w1, [x19, #16]
    and w1, w1, #0xf000
    cmp w1, #0x6000
    b.ne fail
    ldr x1, [x19, #48]
    mov x2, #1
    lsl x2, x2, #20
    cmp x1, x2
    b.ne fail
    mov x0, #1
    adr x1, vda_msg
    adr x2, vda_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #-100
    adr x1, file_path
    mov x2, #0
    mov x3, #0
    mov x8, #56
    svc #0
    cmp x0, #4
    b.ne fail
    mov x26, x0
    mov x0, x26
    mov x1, x19
    mov x2, #16
    mov x8, #63
    svc #0
    cmp x0, #16
    b.ne fail
    mov x0, #1
    mov x1, x19
    mov x2, #16
    mov x8, #64
    svc #0
    cmp x0, #16
    b.ne fail
    mov x0, x26
    mov x8, #57
    svc #0
    cmp x0, #0
    b.ne fail

    mov x0, x23
    mov x8, #57
    svc #0
    cmp x0, #0
    b.ne fail

    mov x0, x19
    mov x8, #160
    svc #0
    cmp x0, #0
    b.ne fail
    ldr x1, [x19]
    movz x2, #0x6967
    movk x2, #0x756e, lsl #16
    movk x2, #0x0078, lsl #32
    cmp x1, x2
    b.ne fail
    add x3, x19, #260
    ldr x1, [x3]
    movz x2, #0x6161
    movk x2, #0x6372, lsl #16
    movk x2, #0x3668, lsl #32
    movk x2, #0x0034, lsl #48
    cmp x1, x2
    b.ne fail
    mov x0, #1
    adr x1, uname_msg
    adr x2, uname_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #0
    mov x1, x19
    mov x8, #113
    svc #0
    cmp x0, #0
    b.ne fail
    ldr x1, [x19]
    movz x2, #0xE100
    movk x2, #0x5E0B, lsl #16
    cmp x1, x2
    b.lo fail
    ldr x1, [x19, #8]
    movz x2, #0xCA00
    movk x2, #0x3B9A, lsl #16
    cmp x1, x2
    b.hs fail
    mov x0, #1
    adr x1, realtime_msg
    adr x2, realtime_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #1
    mov x1, x19
    mov x8, #113
    svc #0
    cmp x0, #0
    b.ne fail
    ldr x1, [x19]
    ldr x3, [x19, #8]
    orr x4, x1, x3
    cbz x4, fail
    cmp x1, #3600
    b.hs fail
    movz x2, #0xCA00
    movk x2, #0x3B9A, lsl #16
    cmp x3, x2
    b.hs fail
    mov x0, #1
    adr x1, monotonic_msg
    adr x2, monotonic_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0

    mov x0, #0
    mov x8, #93
    svc #0

clone_child:
    mov x0, #1
    adr x1, clone_child_msg
    adr x2, clone_child_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0
    mov x0, #42
    mov x8, #93
    svc #0

clone3_child:
    mov x0, #1
    adr x1, clone3_child_msg
    adr x2, clone3_child_msg_end
    sub x2, x2, x1
    mov x8, #64
    svc #0
    mov x0, #7
    mov x8, #93
    svc #0

fail:
    mov x0, #1
    mov x8, #93
    svc #0

write_msg:
    .ascii "ginux-sbsa: controller write ok\n"
write_msg_end:
memory_msg:
    .ascii "ginux-sbsa: controller memory ok\n"
memory_msg_end:
clone_child_msg:
    .ascii "ginux-sbsa: clone child ok\n"
clone_child_msg_end:
clone_parent_msg:
    .ascii "ginux-sbsa: clone parent ok\n"
clone_parent_msg_end:
wait_clone_msg:
    .ascii "ginux-sbsa: waitid clone 42\n"
wait_clone_msg_end:
clone3_child_msg:
    .ascii "ginux-sbsa: clone3 child ok\n"
clone3_child_msg_end:
clone3_parent_msg:
    .ascii "ginux-sbsa: clone3 parent ok\n"
clone3_parent_msg_end:
wait_clone3_msg:
    .ascii "ginux-sbsa: waitid clone3 7\n"
wait_clone3_msg_end:
disk_path:
    .asciz "/dev/vda"
file_path:
    .asciz "/ginux.txt"
vda_msg:
    .ascii "ginux-sbsa: vda block 1048576\n"
vda_msg_end:
uname_msg:
    .ascii "ginux-sbsa: uname aarch64\n"
uname_msg_end:
realtime_msg:
    .ascii "ginux-sbsa: clock realtime\n"
realtime_msg_end:
monotonic_msg:
    .ascii "ginux-sbsa: clock monotonic\n"
monotonic_msg_end:
