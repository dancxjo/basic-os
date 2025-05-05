// restore_context.S

.global restore_context
.type restore_context, @function

restore_context:
    # movb $'X', %al
    # movw $0x3F8, %dx
    # outb %al, %dx

    movq %rdi, %rsp      // load new stack pointer from first arg
    popq %rax
    popq %rbx
    popq %rcx
    popq %rdx
    popq %rbp
    popq %rdi
    popq %rsi
    popq %r8
    popq %r9
    popq %r10
    popq %r11
    popq %r12
    popq %r13
    popq %r14
    popq %r15

    # movb $'X', %al
    # movw $0x3F8, %dx
    # outb %al, %dx

    # movl $0, %eax
    # movl %eax, 0xFEE000B0

    // pop the IRET frame: RIP, CS, RFLAGS, RSP, SS
    iretq
