.intel_syntax noprefix
.global syscall_entry_asm
.text
.type syscall_entry_asm, @function

syscall_entry_asm:
    mov [USER_RSP], rsp
    lea rsp, [SYSCALL_KERNEL_STACK + 16384]

    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    push rcx
    push r11

    mov rcx, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov rdi, rax

    call syscall_entry

    pop r11
    pop rcx

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    mov rsp, [USER_RSP]

    sysretq

.size syscall_entry_asm, . - syscall_entry_asm
