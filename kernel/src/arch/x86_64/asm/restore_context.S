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
    # DEBUG: Entry
    pushq %rax
    pushq %rdx
    movb $'R', %al
    call print_al
    popq %rdx
    popq %rax

    # %rdi points to FullContext
    movq %rdi, %rsp

    # Check CS (offset 128) to see if we are returning to User Mode
    movq 128(%rsp), %rax
    testq $3, %rax
    jnz .user_return

.kernel_return:
    # Returning to Kernel Mode (CS & 3 == 0).
    # iretq will NOT pop RSP/SS, so we must switch stack manually.
    
    # Load Target RSP from Context (offset 144)
    movq 144(%rsp), %rax

    # DEBUG: Print RSP
    pushq %rax
    pushq %rsi
    movq %rax, %rsi
    call print_qword
    popq %rsi
    popq %rax

    
    # We need to construct [RIP, CS, RFLAGS] on the Target Stack.
    # We also push RAX temporarily so we can restore it after stack switch.
    
    # Push RFLAGS (offset 136)
    movq 136(%rsp), %rbx
    movq %rbx, -8(%rax)
    
    # Push CS (offset 128)
    movq 128(%rsp), %rbx
    movq %rbx, -16(%rax)
    
    # Push RIP (offset 120)
    movq 120(%rsp), %rbx
    movq %rbx, -24(%rax)
    
    # Push RAX (offset 112)
    movq 112(%rsp), %rbx
    movq %rbx, -32(%rax)
    
    # Adjust Target RSP to point to the new top (RAX)
    subq $32, %rax
    
    # Restore GPRs (except RAX)
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
    
    # Skip RAX on the Context stack (we pushed it to Target Stack)
    addq $8, %rsp
    
    # Switch to Target Stack
    movq %rax, %rsp
    
    # Restore RAX
    popq %rax
    
    # Stack is now [RIP, CS, RFLAGS]
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

.halt:
    movb $'#', %al
    call print_al
    cli
.hang:
    hlt
    jmp .hang
