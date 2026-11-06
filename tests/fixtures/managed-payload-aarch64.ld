ENTRY(_start)

PHDRS
{
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  bss PT_LOAD FLAGS(6);
}

SECTIONS
{
  . = 0x20000;
  .text : { *(.text*) } :text
  . = ALIGN(0x1000);
  .rodata : { *(.rodata*) } :rodata
  . = ALIGN(0x1000);
  .bss : { *(.bss*) } :bss
  /DISCARD/ : { *(.comment*) *(.note*) *(.eh_frame*) *(.got*) *(.gcc_except_table*) }
}
