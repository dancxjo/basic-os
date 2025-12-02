.section .text
.code64

.global tick_handler
.type tick_handler, @function

.extern rust_schedule_and_switch

# WARNING: This handler runs in interrupt context.
# Do not use full logging macros (info!, kinfo!, etc.) from here or called functions.
# Use klog_irq! or direct serial writes only.
tick_handler:
    # Check if we came from user or kernel
    # Stack: [RIP][CS][RFLAGS]...
    testb $3, 8(%rsp)
    jnz .user_entry

.kernel_entry:
    # We are in Kernel mode. CPU pushed 3 words.
    # Stack: [RIP][CS][RFLAGS]
    # We need to expand to 5 words to match IretFrame layout.
    # Target Stack: [RIP][CS][RFLAGS][RSP][SS]
    
    subq $16, %rsp
    # Stack: [Empty][Empty][RIP][CS][RFLAGS]
    # We want: [RIP][CS][RFLAGS][RSP][SS]

    # Move RIP, CS, RFLAGS down
    pushq %rax # Save RAX temporarily
    
    movq 24(%rsp), %rax  # Load RIP (8+16)
    
    # --- DEBUG: Check if RIP is on stack ---
    # Stack range: 0xffff880010000000 - 0xffff88001000ffff
    # We check if RAX is in this range.
    movq %rax, %rcx
    shrq $16, %rcx
    movabsq $0xffff88001000, %rax
    cmpq %rax, %rcx
    jne .rip_ok
    
    # RIP is on stack! Print 'S'
    movb $'S', %al
    movw $0x3F8, %dx
    outb %al, %dx
    
.rip_ok:
    # ---------------------------------------

    movq 24(%rsp), %rax  # Reload RIP (8+16)
    movq %rax, 8(%rsp)
    
    movq 32(%rsp), %rax  # Load CS (8+24)
    movq %rax, 16(%rsp)
    
    movq 40(%rsp), %rax  # Load RFLAGS (8+32)
    movq %rax, 24(%rsp)
    
    # Calculate original RSP
    # Original RSP = RSP (current) + 8 (pushed RAX) + 16 (sub) + 24 (3 words pushed by CPU) = RSP + 48
    leaq 48(%rsp), %rax
    movq %rax, 32(%rsp)  # Store RSP
    
    movq $0x10, %rax       # SS = 0x10
    movq %rax, 40(%rsp)  # Store SS
    
    popq %rax # Restore RAX
    
    # Now stack has 5 words: [RIP][CS][RFLAGS][RSP][SS]
    # This matches the IretFrame part of FullContext.
    jmp .push_gprs

.user_entry:
    # User mode. CPU pushed 5 words.
    # Stack: [RIP][CS][RFLAGS][RSP][SS]

.push_gprs:
    # Save general-purpose registers
    # This constructs the GeneralRegisters part of FullContext.
    # Layout:
    #   [rsp+0]   = R15
    #   ...
    #   [rsp+112] = RAX
    #   [rsp+120] = RIP (start of IretFrame)
    # Total size: 160 bytes (FullContext)
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

    # Print tick character
    movb $'.', %al
    movw $0x3F8, %dx
    outb %al, %dx

    # Pass pointer to saved registers
    movq %rsp, %rdi
    // IRQ number for timer
    movq $0, %rsi

    // Align RSP to 16 bytes for the call
    andq $~0xF, %rsp
    call rust_schedule_and_switch

    // Should never return
    movb $'X', %al
    movw $0x3F8, %dx
    outb %al, %dx

    hlt
    jmp .

.global yield_now
.type yield_now, @function

yield_now:
    # Save RAX temporarily
    pushq %rax
    # Stack: [Saved RAX][RIP]
    
    subq $32, %rsp
    # Stack: [Gap...][Saved RAX][RIP]
    
    # Move RIP
    movq 40(%rsp), %rax
    movq %rax, 0(%rsp)
    
    # Construct frame
    movq $0x8, 8(%rsp)   # CS
    
    pushfq
    popq %rax
    movq %rax, 16(%rsp)  # RFLAGS
    
    # RSP
    # Original RSP = Current RSP + 32 + 8 (Saved RAX) + 8 (RIP) = RSP + 48
    leaq 48(%rsp), %rax
    movq %rax, 24(%rsp)
    
    # SS
    movq $0x10, 32(%rsp)
    
    # Now stack: [RIP][CS][RFLAGS][RSP][SS][Saved RAX]
    
    # Retrieve Saved RAX
    movq 40(%rsp), %rax
    
    # Push GPRs (RAX first, R15 last)
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

    movq %rsp, %rdi
    // IRQ number for keyboard (we only invoke yield_now from keyboard interrupt)
    movq $1, %rsi
    andq $~0xF, %rsp
    call rust_schedule_and_switch

    hlt
    jmp .
