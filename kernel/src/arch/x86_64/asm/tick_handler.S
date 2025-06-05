.section .text
.code64

.global tick_handler
.type tick_handler, @function

.extern rust_schedule_and_switch

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

    // Print tick character
    movb $'.', %al
    movw $0x3F8, %dx
    outb %al, %dx

    // Dump RSP (64-bit address in hex, 16 nibbles)
    movq %rsp, %rax      // copy rsp to rax
    movq $60, %rcx       // bit shift counter (4 bits per hex digit)
1:
    movq %rax, %rsi
    shrq %cl, %rsi       // shift right to isolate current nibble
    andb $0xF, %sil      // mask to lower 4 bits
    cmpb $10, %sil
    jb 2f
    addb $'A' - 10, %sil
    jmp 3f
2:
    addb $'0', %sil
3:
    movb %sil, %al
    movw $0x3F8, %dx
    outb %al, %dx
    subq $4, %rcx
    jns 1b

    // newline
    movb $'\n', %al
    movw $0x3F8, %dx
    outb %al, %dx

    // Align RSP to 16 bytes
    andq $~0xF, %rsp

    // Pass rsp to Rust scheduler
    movq %rsp, %rdi
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
    # Save registers like in tick_handler
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

    andq $~0xF, %rsp
    movq %rsp, %rdi
    call rust_schedule_and_switch

    hlt
    jmp .
