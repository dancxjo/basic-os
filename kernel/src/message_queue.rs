use alloc::collections::VecDeque;
use alloc::string::String;

/// A simple message queue for logging or communication between components.
#[derive(Debug)]
pub struct MessageQueue {
    queue: VecDeque<String>,
    max_len: usize,
}

impl MessageQueue {
    /// Creates a new message queue with a default max length.
    pub const fn new() -> Self {
        MessageQueue {
            queue: VecDeque::new(),
            max_len: 128,
        }
    }

    /// Push a message onto the queue.
    pub fn push(&mut self, msg: String) {
        if self.queue.len() >= self.max_len {
            self.queue.pop_front(); // Drop the oldest message
        }
        self.queue.push_back(msg);
    }

    /// Pop a message from the front of the queue.
    pub fn pop(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Set a new maximum length.
    pub fn set_max_len(&mut self, max: usize) {
        self.max_len = max;
        while self.queue.len() > self.max_len {
            self.queue.pop_front();
        }
    }

    /// Get the current number of messages.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
