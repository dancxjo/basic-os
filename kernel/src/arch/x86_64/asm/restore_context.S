.section .text
.code64

.global restore_context
.type restore_context, @function

# ----------------------------------------
# restore_context
# Input:
#   %rdi = pointer to IretFrame (inside FullContext)
# Behavior:
#   - loads saved registers from context (at %rdi - 120)
#   - restores stack
#   - executes iretq
# ----------------------------------------
restore_context:
    # %rdi points to IretFrame
    
    # Set RSP to start of FullContext (regs)
    # IretFrame is at offset 120 in FullContext
    movq %rdi, %rsp
    subq $120, %rsp

    # Check CS (offset 8 in IretFrame) to see if we are returning to User Mode
    movq 8(%rdi), %rax
    testq $3, %rax
    jnz .user_return

.kernel_return:
    # Returning to Kernel Mode (CS & 3 == 0).
    # We use lretq instead of iretq to avoid ambiguity about stack popping.
    # lretq pops RIP and CS. We restore RFLAGS manually with popfq.
    
    # Load Target RSP from IretFrame (offset 24) into RAX
    movq 24(%rdi), %rax

    # --- Sanity Check: RSP ---
    testq %rax, %rax
    jz .panic_bad_rsp
    
    # Check for canonical address
    movq %rax, %rcx
    shrq $48, %rcx
    cmpq $0xffff, %rcx
    jne .panic_bad_rsp
    # -------------------------
    
    # Construct frame for lretq: [RIP, CS]
    # And RFLAGS below it for popfq.
    # We want RSP to be RAX after lretq.
    # So we place [RIP, CS] at RAX-16.
    # And RFLAGS at RAX-24.
    
    # [RAX - 8]  = CS (0x8)
    # [RAX - 16] = RIP
    # [RAX - 24] = RFLAGS
    # [RAX - 32] = Saved RAX

    # Set CS to 0x8 (Kernel Code)
    movq $0x8, -8(%rax)
    
    # Copy RIP (offset 0 in IretFrame)
    movq 0(%rdi), %rbx
    movq %rbx, -16(%rax)
    
    # Copy RFLAGS (offset 16 in IretFrame)
    movq 16(%rdi), %rbx
    orq $0x200, %rbx            # Ensure Interrupts Enabled
    movq %rbx, -24(%rax)
    
    # Copy Saved RAX (offset 112 in FullContext)
    movq -8(%rdi), %rbx
    movq %rbx, -32(%rax)

    # Switch to Target Stack (pointing to Saved RAX)
    leaq -32(%rax), %rsp
    
    # Restore RAX
    popq %rax
    
    # Stack is now [RFLAGS, RIP, CS]
    popfq
    # Stack is now [RIP, CS]
    lretq

.panic_bad_rsp:
    movb $'X', %al
    movw $0x3F8, %dx
    outb %al, %dx
    hlt

.panic_bad_rip:
    movb $'R', %al
    movw $0x3F8, %dx
    outb %al, %dx
    hlt

.user_return:
    # Returning to User Mode (CS & 3 == 3).
    # iretq WILL pop RSP/SS automatically.
    
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
    
    # Stack is now [RIP, CS, RFLAGS, RSP, SS]
    iretq
