use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::{collections::VecDeque, vec::Vec};
use core::cell::RefCell;
use log::info;
use wasmi::{Engine, Instance, Module, Store, TypedFunc};

pub type TaskId = usize;

#[derive(Debug, PartialEq, Eq)]
pub enum TaskState {
    Runnable,
    Blocked,
    Finished,
}

pub struct WasmTask {
    pub pid: TaskId,
    pub store: Store<()>,
    pub instance: Instance,
    pub state: TaskState,
}

pub struct Scheduler {
    tasks: Vec<WasmTask>,
    ready_queue: VecDeque<TaskId>,
    current: Option<TaskId>,
}

impl Scheduler {
    const FUEL_QUANTUM: u64 = 1024;

    pub fn new() -> Self {
        Scheduler {
            tasks: Vec::new(),
            ready_queue: VecDeque::new(),
            current: None,
        }
    }

    pub fn add_task(&mut self, task: WasmTask) {
        let pid = task.pid;
        self.tasks.push(task);
        self.ready_queue.push_back(pid);
    }

    pub fn schedule(&mut self) {
        if let Some(pid) = self.ready_queue.pop_front() {
            let task = &mut self.tasks[pid];

            match task.state {
                TaskState::Runnable => {
                    self.current = Some(pid);
                    task.store.set_fuel(Self::FUEL_QUANTUM).unwrap();

                    if let Ok(main_func) =
                        task.instance.get_typed_func::<(), ()>(&task.store, "main")
                    {
                        match main_func.call(&mut task.store, ()) {
                            Ok(_) => {
                                info!("Task {} completed.", pid);
                                task.state = TaskState::Finished;
                            }
                            Err(trap) => {
                                // Check for fuel exhaustion by inspecting the error kind
                                if trap.to_string().contains("all fuel consumed") {
                                    self.ready_queue.push_back(pid);
                                } else {
                                    task.state = TaskState::Finished;
                                    info!("Task {} trapped: {}", pid, trap);
                                }
                            }
                        }
                    } else {
                        info!("Task {} has no 'main' function", pid);
                        task.state = TaskState::Finished;
                    }
                }
                TaskState::Blocked => {
                    info!("Task {} is blocked.", pid);
                    self.ready_queue.push_back(pid);
                }
                TaskState::Finished => {
                    // Do nothing
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ready_queue.is_empty()
    }
}

pub static mut SCHEDULER: Option<Scheduler> = None;
