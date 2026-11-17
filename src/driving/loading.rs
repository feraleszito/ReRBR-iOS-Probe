//! One active resource job and at most one replacement; cancelled results never install.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

type Task<T> = Box<dyn FnOnce(&AtomicBool) -> Result<T, String> + Send>;
struct Job<T> {
    id: u64,
    receive: mpsc::Receiver<Result<T, String>>,
    cancel: Arc<AtomicBool>,
}
pub(crate) struct Loader<T> {
    next_id: u64,
    active_id: u64,
    job: Option<Job<T>>,
    queued: Option<(u64, Task<T>)>,
}
impl<T> Default for Loader<T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            active_id: 0,
            job: None,
            queued: None,
        }
    }
}
impl<T: Send + 'static> Loader<T> {
    pub fn request(&mut self, task: Task<T>) {
        self.cancel();
        self.next_id += 1;
        let id = self.next_id;
        self.active_id = id;
        if self.job.is_some() {
            self.queued = Some((id, task));
        } else {
            self.launch(id, task);
        }
    }
    fn launch(&mut self, id: u64, task: Task<T>) {
        let (send, receive) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        // Dropping a JoinHandle detaches; no UI-thread wait during cancellation.
        let result = std::thread::Builder::new()
            .name("rally-resource-load".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task(&flag)))
                    .unwrap_or_else(|_| Err("resource worker panicked".into()));
                let _ = send.send(result);
            });
        if let Err(error) = result {
            let (send, replacement) = mpsc::channel();
            let _ = send.send(Err(format!("resource worker: {error}")));
            self.job = Some(Job {
                id,
                receive: replacement,
                cancel,
            });
        } else {
            self.job = Some(Job {
                id,
                receive,
                cancel,
            });
        }
    }
    pub fn poll(&mut self) -> Option<Result<T, String>> {
        let job = self.job.as_ref()?;
        let result = match job.receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("resource worker disconnected".into()),
        };
        let cancelled = job.id != self.active_id || job.cancel.load(Ordering::Relaxed);
        self.job = None;
        if let Some((id, task)) = self.queued.take() {
            self.launch(id, task);
        }
        if cancelled {
            None
        } else {
            Some(result)
        }
    }
}
impl<T> Loader<T> {
    pub fn cancel(&mut self) {
        self.next_id += 1;
        self.active_id = self.next_id;
        self.queued = None;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl<T> Drop for Loader<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn loading_keeps_ui_pollable_and_discards_cancelled_resources() {
        let (finish, wait) = mpsc::channel();
        let mut loader = Loader::default();
        loader.request(Box::new(move |_| {
            wait.recv().unwrap();
            Ok(1)
        }));
        for _ in 0..100 {
            assert!(loader.poll().is_none());
        }
        loader.request(Box::new(|_| Ok(2)));
        loader.request(Box::new(|_| Ok(3)));
        finish.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = loader.poll() {
                assert_eq!(result.unwrap(), 3);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        loader.cancel();
        assert!(loader.poll().is_none());

        // Stale completion that finishes after cancel is never installed.
        let (stale_tx, stale_rx) = mpsc::channel();
        loader.request(Box::new(move |_| {
            stale_rx.recv().unwrap();
            Ok(99)
        }));
        loader.cancel();
        stale_tx.send(()).unwrap();
        let wait_deadline = Instant::now() + Duration::from_secs(5);
        while loader.job.is_some() {
            assert!(loader.poll().is_none());
            assert!(Instant::now() < wait_deadline);
            std::thread::yield_now();
        }
    }
}
