.global syscall_entry_asm
.type syscall_entry_asm, @function

syscall_entry_asm:
    //  TODO: Save volatile registers if needed (if not using interrupt return)

    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    // We assume rax, rdi, rsi, rdx are syscall args

    // Call into Rust
    call syscall_entry

    // Return to userspace
    sysretq
