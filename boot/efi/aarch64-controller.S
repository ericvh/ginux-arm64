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
