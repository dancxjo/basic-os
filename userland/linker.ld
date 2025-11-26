ENTRY(_start)

SECTIONS {
    . = 0x400000000000;

    .text : ALIGN(4K) {
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss : ALIGN(4K) {
        *(.bss .bss.* COMMON)
        *(.lbss .lbss.*)
    }
}
