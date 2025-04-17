/* hello-user.ld */
ENTRY(_start)

SECTIONS {
  . = 0x200;
  .text : {
    *(.text._start)
    *(.text*)
  }
  .rodata : { *(.rodata*) }
  .data : { *(.data*) }
  .bss : {
    *(.bss*)
    *(COMMON)
  }
}
