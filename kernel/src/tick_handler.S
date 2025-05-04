.global tick_handler
.type tick_handler, @function

.extern spy_context
.extern dummy_task

.section .text
tick_handler:

    // Save general-purpose registers
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

    // At this point:
    // [RSP + 0]  = r15
    // [RSP + 8]  = r14
    // ...
    // [RSP + 112] = rax
    // [RSP + 120] = return RIP
    // [RSP + 128] = CS
    // [RSP + 136] = RFLAGS

    // Capture current context
    movq %rsp, %rdi
    call spy_context

    // Allocate new stack manually (1 page)
    movq $4096, %rdi
    call alloc_stack       // assume: alloc_stack(len) -> %rax = base ptr
                           // or replace this with a pointer to static memory

    // rax = base of new stack
    addq $4096, %rax       // stack_top = base + 4096
    movq %rax, %rsi        // rsi = new_rsp

    // We need to reserve space for:
    // - 15 GPRs
    // - RIP, CS, RFLAGS
    subq $8*18, %rsi       // new rsp = stack_top - (18*8)

    // Copy 18 * 8 bytes from current RSP to rsi
    movq %rsp, %rdi        // rdi = old rsp
    movq $18, %rcx         // number of qwords to copy
.copy_loop:
    movq (%rdi), %rdx
    movq %rdx, (%rsi)
    addq $8, %rdi
    addq $8, %rsi
    loop .copy_loop

    // Now rsi points to end of copied region, back it up
    subq $8*18, %rsi       // rsi = new_rsp (again)

    // Overwrite new RIP with dummy_task
    movq $dummy_task, %rdx
    movq %rdx, 15*8(%rsi)  // offset for RIP

    // Load new stack and iretq
    movq %rsi, %rsp
    iretq
