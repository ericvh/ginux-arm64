ENTRY(_start)

PHDRS
{
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  bss PT_LOAD FLAGS(6);
}

/* arm64 kernels run with 4K, 16K, or 64K pages. Segments with different
 * protections are a largest page apart, so each lands on its own pages on
 * any of them. */
SECTIONS
{
  . = 0x20000;
  .text : { *(.text*) } :text
  . = ALIGN(0x10000);
  .rodata : { *(.rodata*) } :rodata
  . = ALIGN(0x10000);
  .bss : { *(.bss*) } :bss
  /DISCARD/ : { *(.comment*) *(.note*) *(.eh_frame*) *(.got*) *(.gcc_except_table*) }
}
