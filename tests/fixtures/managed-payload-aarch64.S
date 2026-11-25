.section .text
.balign 4
.global _start
_start:
    mov x8, #172
    svc #0
    mov x9, #4242
    cmp x0, x9
    b.ne 1f

    mov x8, #64
    mov x0, #1
    adr x1, bypass_marker
    mov x2, #25
    svc #0
    cmp x0, #25
    b.ne 1f

    mov x8, #160
    adr x0, uname_buffer
    svc #0
    cmn x0, #1
    b.ne 1f

    mov x8, #93
    mov x0, xzr
    svc #0
1:
    mov x8, #93
    mov x0, #1
    svc #0

.section .rodata
.balign 8
bypass_marker:
    .ascii "UNMEDIATED_PAYLOAD_WRITE\n"

.section .bss
.balign 8
uname_buffer:
    .skip 390
