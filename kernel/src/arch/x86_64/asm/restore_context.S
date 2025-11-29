.section .text
.code64

.global restore_context
.type restore_context, @function

# ----------------------------------------
# print_al: print character in AL to COM1
# ----------------------------------------
print_al:
    movw $0x3F8, %dx
    outb %al, %dx
    ret

# ----------------------------------------
# print_qword: print 64-bit value in RSI (16 hex digits)
# ----------------------------------------
print_qword:
    mov $60, %rcx         # shift count: highest nibble first
.print_nibble:
    movq %rsi, %rax
    shr %cl, %rax
    and $0xF, %al
    cmp $10, %al
    jb .digit
    add $'A' - 10, %al
    jmp .emit
.digit:
    add $'0', %al
.emit:
    call print_al
    sub $4, %rcx
    jns .print_nibble
    ret

# ----------------------------------------
# restore_context
# Input:
#   %rdi = pointer to GeneralRegisters struct (context_ptr)
# Behavior:
#   - loads saved registers from context
#   - restores stack
#   - executes iretq
# ----------------------------------------
restore_context:
    # Mark entry
    # movb $'$', %al
    # call print_al

    # Set stack pointer to saved context
    movq %rdi, %rsp       # rdi points to context buffer

    # Check CS (offset 128) to see if we are returning to User or Kernel
    movq 128(%rsp), %rax
    testb $3, %al
    jz .kernel_restore

.user_restore:
    # Restore general-purpose registers from context
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

    # Print marker before iretq
    # movb $'>', %al
    # call print_al

    iretq

.kernel_restore:
    # We are returning to Kernel Mode.
    # iretq will pop [RIP, CS, RFLAGS] but NOT [RSP, SS].
    # So we must manually switch to the target stack.
    # We construct an iretq frame [RIP, CS, RFLAGS] on the target stack.

    # Target RSP is at offset 144
    movq 144(%rsp), %rax

    # Copy RIP (120), CS (128), RFLAGS (136) to Target Stack
    # Target Stack Layout:
    # [ ... ] <--- Target RSP
    # [ RIP ] <--- Target RSP - 24
    # [ CS  ] <--- Target RSP - 16
    # [RFLAGS]<--- Target RSP - 8
    # Wait, stack grows down.
    # Top of stack (lowest address) should be RIP.
    # So:
    # -24: RIP
    # -16: CS
    # -8:  RFLAGS
    
    movq 120(%rsp), %rbx
    movq %rbx, -24(%rax)

    movq 128(%rsp), %rbx
    movq %rbx, -16(%rax)

    movq 136(%rsp), %rbx
    movq %rbx, -8(%rax)

    # Update the RSP slot in the Context Buffer to point to the new stack top
    subq $24, %rax
    movq %rax, 144(%rsp)

    # Restore GPRs
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

    # Switch to the target stack
    # The RSP slot (offset 144) was updated to point to the new stack top.
    # Since we popped 15 regs (120 bytes), rsp is now at offset 120 (RIP).
    # So offset 144 is now at 24(%rsp).
    movq 24(%rsp), %rsp

    # Now rsp points to [RIP, CS, RFLAGS] on the target kernel stack.
    iretq

.halt:
    movb $'#', %al
    call print_al
    cli
.hang:
    hlt
    jmp .hang
