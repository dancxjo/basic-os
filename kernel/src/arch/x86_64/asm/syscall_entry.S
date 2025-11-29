.intel_syntax noprefix
.global syscall_entry_asm
.text
.type syscall_entry_asm, @function

syscall_entry_asm:
    # Interrupts are disabled here (SFMASK)

    # Save user RSP temporarily
    mov [USER_RSP], rsp
    
    # Switch to per-task kernel stack
    mov rsp, [KERNEL_STACK_PTR]

    # Push user RSP onto kernel stack so it survives context switches
    push [USER_RSP]

    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    push rcx
    push r11

    mov r8, r10
    mov rcx, rdx
    mov rdx, rsi
    mov rsi, rdi
    mov rdi, rax

    # Enable interrupts to allow preemption/blocking
    sti

    call syscall_entry

    # Disable interrupts before restoring stack
    cli

    pop r11
    pop rcx

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    # Restore user RSP from stack
    pop rsp

    sysretq

.size syscall_entry_asm, . - syscall_entry_asm
