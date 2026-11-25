ENTRY(_start)
PHDRS
{
    text PT_LOAD FLAGS(5);
}
SECTIONS
{
    . = 0x20000000;
    .text : {
        *(.text .text.*)
    } :text
    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.ARM.attributes)
        *(.data)
        *(.bss)
    }
}
