.global switch_to_task
.type switch_to_task, @function

# Takes: rdi = pointer to stack containing [RIP, CS, RFLAGS]

switch_to_task:
    mov %rdi, %rsp      # switch to new task's stack
    iretq               # return directly into task's context
