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
    movb $'$', %al
    call print_al

    # Set stack pointer to saved context
    movq %rdi, %rsp       # rdi points to context buffer

    # Restore general-purpose registers from context
    # Order must match GeneralRegisters struct layout:
    # r15 at offset 0, r14 at offset 8, ..., rax at offset 112
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
    movb $'>', %al
    call print_al

    # RSP now points to the saved IretFrame: [rip][cs][rflags][rsp][ss]
    # If returning to ring 0, the CPU won't pop rsp/ss, so move to the
    # recorded kernel stack pointer and rebuild a minimal frame.
    movq 8(%rsp), %rax        # saved CS
    testb $0x3, %al
    jnz 1f                    # non-zero CPL -> user return, fall through

    movq 0(%rsp), %rcx        # saved RIP
    movq 16(%rsp), %rsi       # saved RFLAGS
    movq 24(%rsp), %rdi       # saved RSP
    movq %rdi, %rsp           # switch to the saved kernel stack

    pushq %rsi                # rflags
    pushq %rax                # cs
    pushq %rcx                # rip
    iretq

1:
    iretq

.halt:
    movb $'#', %al
    call print_al
    cli
.hang:
    hlt
    jmp .hang
