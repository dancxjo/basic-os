.global switch_to_task
.type switch_to_task, @function

switch_to_task:
    mov %rsp, %rdi
    call check_alignment

    iretq
