.global switch_context
.type switch_context, @function

// Arguments:
// rdi = pointer to old stack pointer (*mut usize)
// rsi = new stack pointer (*const usize)

switch_context:
    // Save old stack pointer into *rdi
    movq %rsp, (%rdi)

    // Load new stack pointer from rsi
    movq %rsi, %rsp

    // Return — resume from the new stack
    ret
