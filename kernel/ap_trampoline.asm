; ap_trampoline.asm — relocatable with external reference to ap_main
BITS 16
SECTION .text
GLOBAL ap_trampoline

ap_trampoline:
    cli
    cld

    ; Load GDT
    lgdt [gdt_ptr]

    ; Enter protected mode
    mov eax, cr0
    or eax, 0x1
    mov cr0, eax

    jmp 0x08:protected_mode_entry

; --------------- GDT ----------------
SECTION .data
align 8
gdt_start:
    dq 0x0000000000000000
    dq 0x00CF9A000000FFFF

gdt_ptr:
    dw gdt_end - gdt_start - 1
    dd gdt_start
gdt_end:

; -------- 32-bit code section --------
[BITS 32]
SECTION .text
protected_mode_entry:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov esp, 0x9000

    jmp 0x200000

halt:
    hlt
    jmp halt
