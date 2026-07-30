#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_processor_cannot_hold_the_same_queue_lock() {
        let dir = std::env::temp_dir().join(format!(
            "boos-queue-lock-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lock_path = dir.join(".processor.lock");

        let first = QueueProcessorLock::acquire(&lock_path).unwrap();
        let second = QueueProcessorLock::acquire(&lock_path).unwrap();

        assert!(first.is_some());
        assert!(second.is_none(), "two processors must not execute the queue");

        drop(first);
        assert!(QueueProcessorLock::acquire(&lock_path).unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
