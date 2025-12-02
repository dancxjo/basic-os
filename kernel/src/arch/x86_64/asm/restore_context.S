.section .text
.code64

.global restore_context
.type restore_context, @function

# ----------------------------------------
# restore_context
# Input:
#   %rdi = pointer to FullContext
# Behavior:
#   - loads saved registers from context (at %rdi)
#   - restores stack
#   - executes iretq
# ----------------------------------------
restore_context:
    # %rdi points to FullContext
    # Layout:
    #   at rdi + 0    : GeneralRegisters (15x8 = 120 bytes)
    #   at rdi + 120  : IretFrame { rip, cs, rflags, rsp, ss } (5x8 = 40 bytes)
    
    # Set RSP to start of FullContext (regs)
    movq %rdi, %rsp

    # Check CS (offset 8 in IretFrame -> 120 + 8 = 128)
    movq 128(%rdi), %rax
    testq $3, %rax
    jnz .user_return

.kernel_return:
    # Returning to Kernel Mode (CS & 3 == 0).
    # We use lretq instead of iretq to avoid ambiguity about stack popping.
    # lretq pops RIP and CS. We restore RFLAGS manually with popfq.
    
    # Load Target RSP from IretFrame (offset 24 -> 120 + 24 = 144)
    movq 144(%rdi), %rax

    # --- Sanity Check: RSP ---
    testq %rax, %rax
    jz .panic_bad_rsp
    
    # Check for canonical address
    movq %rax, %rcx
    shrq $48, %rcx
    cmpq $0xffff, %rcx
    jne .panic_bad_rsp
    # -------------------------
    
    # --- Sanity Check: RIP ---
    # Load RIP (offset 0 in IretFrame -> 120 + 0 = 120)
    movq 120(%rdi), %rcx
    
    # Check if RIP is in kernel text (starts with 0xffffffff80...)
    movq %rcx, %rdx
    shrq $32, %rdx
    movl $0xffffffff, %ebx
    cmpq %rbx, %rdx
    jne .panic_bad_rip
    # -------------------------

    # We want RSP to be RAX after lretq.
    # So we place [RIP, CS] at RAX-16.
    # And RFLAGS at RAX-24.
    
    # [RAX - 8]  = CS (0x8)
    # [RAX - 16] = RIP
    # [RAX - 24] = RFLAGS
    # [RAX - 32] = Saved RAX

    # Set CS to 0x8 (Kernel Code)
    movq $0x8, -8(%rax)
    
    # Copy RIP (offset 120 in FullContext)
    movq 120(%rdi), %rbx
    movq %rbx, -16(%rax)
    
    # Copy RFLAGS (offset 136 in FullContext)
    movq 136(%rdi), %rbx
    orq $0x200, %rbx            # Ensure Interrupts Enabled
    movq %rbx, -24(%rax)
    
    # Copy Saved RAX (offset 112 in FullContext)
    movq 112(%rdi), %rbx
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
    
    # We need to restore registers from FullContext (at RSP)
    # Then execute iretq.
    # But iretq expects stack: [RIP, CS, RFLAGS, RSP, SS]
    # Our stack (FullContext) ends at offset 120.
    # IretFrame starts at offset 120.
    # So if we pop all registers, RSP will point to IretFrame.
    
    # Restore all GPRs (except RSP, which is just the stack pointer)
    # The pop sequence matches GeneralRegisters layout (reverse of push)
    
    # Layout: r15, r14, ..., rax
    # popq pops from low to high.
    # So we pop r15, then r14, ...
    
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
    
    # Now RSP points to IretFrame [RIP, CS, RFLAGS, RSP, SS]
    iretq
    
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
