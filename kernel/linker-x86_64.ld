/* Tell the linker that we want an x86_64 ELF64 output file */
OUTPUT_FORMAT(elf64-x86-64)

/* We want the symbol kmain to be our entry point */
ENTRY(kmain)

/* Define the program headers we want so the bootloader gives us the right */
/* MMU permissions; this also allows us to exert more control over the linking */
/* process. */
PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    /* Place the kernel in the higher half: 0xffffffff80000000 */
    . = 0xffffffff80000000;

    .text : {
        _start = .;
        *(.text .text.*)
        _etext = .;
    } :text

    /* Move to the next memory page for .rodata */
    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Move to the next memory page for .data */
    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .data : {
        _sdata = .;
        *(.data .data.*)

        /* Place the sections that contain the Limine requests as part of the .data */
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
        _edata = .;
    } :data

    /* NOTE: .bss needs to be the last thing mapped to :data */
    .bss : {
        _sbss = .;
        *(.bss .bss.*)
        *(COMMON)
        _end = .;
    } :data

    /* Discard .note.* and .eh_frame* */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
