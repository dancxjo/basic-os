//! Task management module.
//!
//! This module provides the core task management infrastructure including:
//! - Context switching between tasks (save/restore CPU state)
//! - Task scheduling (round-robin scheduling via timer interrupts)
//! - ELF executable loading for user-space tasks
//!
//! The scheduler is driven by the APIC timer interrupt which triggers
//! context switches between tasks. Each task has its own stack and
//! saved context (registers + interrupt frame).

pub mod context;
pub mod executable;
pub mod scheduler;
