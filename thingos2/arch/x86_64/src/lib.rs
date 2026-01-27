//! Architecture glue for x86_64. This crate is a thin veneer around
//! `thingos2-kernel` that documents how the Limine boot path hands control to
//! the graph-native kernel.

use thingos2_kernel::{Kernel, TaskId};

/// Boot-time handle that wires architecture bring-up into the generic kernel.
#[derive(Debug, Default)]
pub struct ArchKernel {
    pub kernel: Kernel,
}

impl ArchKernel {
    pub fn new() -> Self {
        Self {
            kernel: Kernel::new(),
        }
    }

    /// Placeholder for setting up descriptor tables, paging, and timer interrupts.
    pub fn initialize_platform(&mut self) {
        // The real implementation will configure GDT/IDT/TSS, enable paging, and
        // install timer + interrupt handlers that preempt into the scheduler.
    }

    /// Launch built-in bundles after boot modules are staged.
    pub fn launch_boot_bundles(&mut self) -> Vec<TaskId> {
        let mut tasks = Vec::new();
        let drivers = ["framebuffer", "keyboard", "mouse", "compositor", "demo-app"];
        for name in drivers {
            let (_, task) = self.kernel.launch_bundle(name);
            tasks.push(task);
        }
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_kernel_boots_tasks() {
        let mut arch = ArchKernel::new();
        arch.initialize_platform();
        let tasks = arch.launch_boot_bundles();
        assert_eq!(tasks.len(), 5);
        assert_eq!(tasks[0], TaskId(0));
    }
}
