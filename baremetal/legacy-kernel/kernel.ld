ENTRY(_start)

PHDRS {
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
    user PT_LOAD FLAGS(5);
}

SECTIONS {
    . = 0x40080000;

    .text : {
        KEEP(*(.text.boot))
        . = ALIGN(2048);
        KEEP(*(.text.vectors))
        *(.text .text.*)
    } :text

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } :data

    . = ALIGN(16);
    stack_base = .;
    . += 0x4000;
    stack_top = .;

    ASSERT(stack_top <= 0x40200000, "kernel image crosses the user mapping")

    . = 0x40200000;
    .usertext : {
        KEEP(*(.usertext))
    } :user

    user_stack_top = 0x40404000;
}
