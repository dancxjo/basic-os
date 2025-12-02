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
    # We switch to the Target Stack manually to ensure each task runs on its own stack.
    # We construct a 5-word frame [RIP, CS, RFLAGS, RSP, SS] to be safe.
    
    # Load Target RSP from IretFrame (offset 24) into RAX
    movq 24(%rdi), %rax
    
    # We need to construct [RIP, CS, RFLAGS, RSP, SS] on the Target Stack.
    # Target Stack Top is in RAX.
    # [RAX - 8]  = SS
    # [RAX - 16] = RSP (Target RSP)
    # [RAX - 24] = RFLAGS
    # [RAX - 32] = CS
    # [RAX - 40] = RIP
    # [RAX - 48] = Saved RAX (temp)

    # Copy SS (offset 32 in IretFrame)
    movq 32(%rdi), %rbx
    movq %rbx, -8(%rax)

    # Copy RSP (offset 24 in IretFrame) - use RAX
    movq %rax, -16(%rax)

    # Copy RFLAGS (offset 16 in IretFrame)
    movq 16(%rdi), %rbx
    movq %rbx, -24(%rax)
    
    # Copy CS (offset 8 in IretFrame)
    movq 8(%rdi), %rbx
    movq %rbx, -32(%rax)
    
    # Copy RIP (offset 0 in IretFrame)
    movq 0(%rdi), %rbx
    movq %rbx, -40(%rax)
    
    # Copy Saved RAX (offset 112 in FullContext / regs)
    # This is still relative to RSP (which points to FullContext)
    movq 112(%rsp), %rbx
    movq %rbx, -48(%rax)
    
    # Restore GPRs (except RAX) from the Context struct
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
    
    # Skip RAX on the Context stack (we manually copied it to Target Stack)
    addq $8, %rsp
    
    # Switch to Target Stack (pointing to Saved RAX)
    leaq -48(%rax), %rsp
    
    # Restore RAX
    popq %rax
    
    # Stack is now [RIP, CS, RFLAGS, RSP, SS]
    # RSP is (TargetRSP - 40)
    iretq

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
