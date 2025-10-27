use core::sync::atomic::{AtomicPtr, Ordering};
use core::ptr;
use alloc::boxed::Box;

// A lock-free stack implementation using Treiber's algorithm
// This allows multiple threads to push and pop without locks
pub struct AtomicStack<T> {
    head: AtomicPtr<Node<T>>,
}

struct Node<T> {
    value: T,
    next: *mut Node<T>,
}

impl<T> AtomicStack<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn push(&self, value: T) {
        let new_node = Box::into_raw(Box::new(Node {
            value,
            next: ptr::null_mut(),
        }));

        loop {
            let head = self.head.load(Ordering::Acquire);
            unsafe {
                (*new_node).next = head;
            }

            if self.head
                .compare_exchange(head, new_node, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);

            if head.is_null() {
                return None;
            }

            let next = unsafe { (*head).next };

            if self.head
                .compare_exchange(head, next, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                let node = unsafe { Box::from_raw(head) };
                return Some(node.value);
            }
        }
    }

    pub fn drain_to_vec(&self) -> alloc::vec::Vec<T> {
        let mut vec = alloc::vec::Vec::new();
        while let Some(item) = self.pop() {
            vec.push(item);
        }
        vec
    }

    pub fn push_from_iter<I: IntoIterator<Item = T>>(&self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }
}

impl<T> Drop for AtomicStack<T> {
    fn drop(&mut self) {
        // Clean up all remaining nodes
        while self.pop().is_some() {}
    }
}

// Safety: AtomicStack can be safely shared between threads
unsafe impl<T: Send> Send for AtomicStack<T> {}
unsafe impl<T: Send> Sync for AtomicStack<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let stack = AtomicStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);

        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_is_empty() {
        let stack = AtomicStack::new();
        assert!(stack.pop().is_none());

        stack.push(42);

        assert!(stack.pop().is_some());
        assert!(stack.pop().is_none());
    }

    #[test]
    fn test_drain_to_vec() {
        let stack = AtomicStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);

        let vec = stack.drain_to_vec();
        assert_eq!(vec.len(), 3);
        assert!(stack.pop().is_none());
    }
}
