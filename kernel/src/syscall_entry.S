.intel_syntax noprefix
.global syscall_entry_asm
.text
.type syscall_entry_asm, @function

syscall_entry_asm:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    call syscall_entry

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    sysretq

.size syscall_entry_asm, . - syscall_entry_asm
