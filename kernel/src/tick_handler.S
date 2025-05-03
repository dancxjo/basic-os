.global tick_handler
.type tick_handler, @function

tick_handler:

gather_context:
    call spy_context

    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rbp
    pushq %rdi
    pushq %rsi
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    movq %rsp, %rdi          # rdi = &saved_regs
    call schedule            # rax = new_rsp

    cmp %rax, %rdi           # switching to a new stack?
    jne restore_stack
    jmp rollback             # else resume same task

restore_stack:
    movq %rax, %rsp          # switch to new task's stack
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsi
    popq %rdi
    popq %rbp
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq                    # enter new task

rollback:
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsi
    popq %rdi
    popq %rbp
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax
    iretq                    # return to interrupted task

