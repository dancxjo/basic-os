OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    . = 0xffffffff80000000;

    .text : {
        *(.text .text.*)
    } :text

    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .data : {
        *(.data .data.*)
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /* Trampoline target — outside loaded image — no PHDR */
    . = 0xffffffff82000000;
    .ap_main : AT(0x200000) {
        KEEP(*(.ap_entry))
    }
    /* no :text here — do not assign to a PT_LOAD PHDR! */

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
