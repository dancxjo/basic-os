ENTRY(ap_trampoline)

SECTIONS {
    . = 0x7000;

    .text : {
        *(.text*)
    }

    .data : {
        *(.data*)
    }

    /DISCARD/ : {
        *(.comment)
        *(.note*)
    }
}
OUTPUT_FORMAT("binary")
