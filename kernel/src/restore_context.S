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

    # Load saved RSP from context_ptr (first 8 bytes)
    movq (%rdi), %rsi     # rsi = *(context_ptr) = intended rsp

    # Log it
    movb $'R', %al
    call print_al
    movb $':', %al
    call print_al
    call print_qword
    movb $'\n', %al
    call print_al

    # Optionally validate RSP canonicality
    movq %rsi, %rax
    shr $48, %rax
    cmp $0xFFFF, %rax
    je .ok_rsp
    cmp $0x0000, %rax
    je .ok_rsp
    movb $'!', %al        # bad RSP
    call print_al
    jmp .halt

.ok_rsp:
    # Load stack pointer
    movq %rdi, %rsp       # rdi points to context buffer

    # Restore general-purpose registers from context
    popq %rax
    popq %rbx
    popq %rcx
    popq %rdx
    popq %rbp
    popq %rdi
    popq %rsi
    popq %r8
    popq %r9
    popq %r10
    popq %r11
    popq %r12
    popq %r13
    popq %r14
    popq %r15

    # Print marker before iretq
    movb $'>', %al
    call print_al

    iretq

.halt:
    movb $'#', %al
    call print_al
    cli
.hang:
    hlt
    jmp .hang
