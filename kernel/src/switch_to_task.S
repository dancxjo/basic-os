.global switch_to_task
.type switch_to_task, @function

switch_to_task:
    movq %rdi, %rsp         // Load task’s fabricated stack

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

    // Spy on final state before jumping to task entry
    movq %rsp, %rdi         // Current stack pointer passed to spy_context
    ; call spy_context        // extern "C" fn spy_context(rsp: *const u64)

    ; sti                     // Enable interrupts

    ret                     // Jump to task (RIP was top of stack)
