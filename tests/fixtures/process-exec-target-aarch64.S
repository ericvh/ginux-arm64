/* aarch64 form of process-exec-target.S: write (64), then exit (93). */
.global _start
_start:
 mov x8, #64
 mov x0, #1
 adr x1, text
 mov x2, #13
 svc #0
 mov x8, #93
 mov x0, #0
 svc #0
text: .ascii "exec-success\n"
