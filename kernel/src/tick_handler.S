// tick_handler.S — Assembly tick interrupt handler for context switching

.global tick_handler
.type tick_handler, @function

.extern rust_schedule_and_switch

// This is the interrupt handler that gets called on each timer tick
// Saves CPU state, calls scheduler, and returns to next task context

tick_handler:
    // Save general-purpose registers (callee-saved and volatile)
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

    // Save stack pointer to pass to scheduler
    movq %rsp, %rdi

    // Call into Rust-side scheduler
    call rust_schedule_and_switch

    // The function above never returns; it jumps via iretq to a new context
    hlt
    jmp .
