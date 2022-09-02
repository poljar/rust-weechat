pub use async_task::{Runnable, Task};
use futures::future::{BoxFuture, Future};
use pipe_channel::{channel, Receiver, Sender};
use std::{
    collections::VecDeque,
    panic,
    sync::{Arc, Mutex},
};

use crate::{
    hooks::{FdHook, FdHookCallback, FdHookMode},
    Weechat,
};

static mut _EXECUTOR: Option<WeechatExecutor> = None;

type BufferName = String;

type Job = Runnable;
struct BufferJob(Runnable, BufferName);

impl BufferJob {
    fn run(self) -> bool {
        self.0.run()
    }

    fn cancel(self) {
        drop(self)
    }

    fn tag(&self) -> &BufferName {
        &self.1
    }
}

enum ExecutorJob {
    Job(Job),
    BufferJob(BufferJob),
}

type FutureQueue = Arc<Mutex<VecDeque<ExecutorJob>>>;

#[derive(Clone)]
pub struct WeechatExecutor {
    _hook: Arc<Mutex<Option<FdHook<Receiver<()>>>>>,
    sender: Arc<Mutex<Sender<()>>>,
    futures: FutureQueue,
    non_local_futures: Arc<Mutex<VecDeque<BoxFuture<'static, ()>>>>,
}

impl FdHookCallback for WeechatExecutor {
    type FdObject = Receiver<()>;

    fn callback(&mut self, _weechat: &Weechat, receiver: &mut Receiver<()>) {
        if receiver.recv().is_err() {
            return;
        }

        let future = self.futures.lock().unwrap().pop_front();

        // Run a local future if there is one.
        if let Some(task) = future {
            match task {
                ExecutorJob::Job(t) => {
                    let _ = panic::catch_unwind(|| t.run());
                }
                ExecutorJob::BufferJob(t) => {
                    let weechat = unsafe { Weechat::weechat() };
                    let buffer_name = t.tag();

                    let buffer = weechat.buffer_search("==", buffer_name);

                    if buffer.is_some() {
                        let _ = panic::catch_unwind(|| t.run());
                    } else {
                        t.cancel()
                    }
                }
            }
        }

        let future = self.non_local_futures.lock().unwrap().pop_front();
        // Spawn a future if there was one sent from another thread.
        if let Some(future) = future {
            self.spawn_local(future).detach();
        }
    }
}

impl WeechatExecutor {
    fn new() -> Self {
        let (sender, receiver) = channel();
        let sender = Arc::new(Mutex::new(sender));
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let non_local = Arc::new(Mutex::new(VecDeque::new()));

        let executor = WeechatExecutor {
            _hook: Arc::new(Mutex::new(None)),
            sender,
            futures: queue,
            non_local_futures: non_local,
        };

        let hook = FdHook::new(receiver, FdHookMode::Read, executor.clone())
            .expect("Can't create executor FD hook");

        *executor._hook.lock().unwrap() = Some(hook);

        executor
    }

    pub fn spawn_local<F>(&self, future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let sender = Arc::downgrade(&self.sender);
        let queue = Arc::downgrade(&self.futures);

        let schedule = move |runnable| {
            let sender = sender.upgrade();
            let queue = queue.upgrade();

            if let Some(q) = queue {
                let sender = sender.expect("Futures queue exists but the channel got dropped");
                let mut weechat_notify = sender
                    .lock()
                    .expect("Weechat notification sender lock is poisoned");

                let mut queue = q
                    .lock()
                    .expect("Lock of the future queue of the Weechat executor is poisoned");

                queue.push_back(ExecutorJob::Job(runnable));
                weechat_notify
                    .send(())
                    .expect("Can't notify Weechat to run a future");
            }
        };

        let (runnable, task) = async_task::spawn_local(future, schedule);

        runnable.schedule();

        task
    }

    pub fn free() {
        unsafe {
            _EXECUTOR.take();
        }
    }

    pub fn start() {
        let executor = WeechatExecutor::new();
        unsafe {
            _EXECUTOR = Some(executor);
        }
    }

    /// Spawn a local Weechat future from the non-main thread.
    pub fn spawn_from_non_main<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let executor = unsafe { _EXECUTOR.as_ref().expect("Executor wasn't started") };

        let future = Box::pin(future);
        let mut queue = executor.non_local_futures.lock().unwrap();
        queue.push_back(future);
        executor
            .sender
            .lock()
            .unwrap()
            .send(())
            .expect("Can't notify Weechat to spawn a non-local future");
    }

    /// Spawn a future that will run on the Weechat main loop.
    pub fn spawn<F>(future: F) -> Option<Task<F::Output>>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let executor = unsafe { _EXECUTOR.as_ref() };

        if let Some(executor) = executor {
            Some(executor.spawn_local(future))
        } else {
            None
        }
    }

    pub(crate) fn spawn_buffer_cb<F>(buffer_name: String, future: F) -> Task<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let executor = unsafe { _EXECUTOR.as_ref().expect("Executor wasn't started") };

        let sender = Arc::downgrade(&executor.sender);
        let queue = Arc::downgrade(&executor.futures);

        let schedule = move |runnable| {
            let sender = sender.upgrade();
            let queue = queue.upgrade();

            if let Some(q) = queue {
                let sender = sender.expect("Futures queue exists but the channel got dropped");
                let mut weechat_notify = sender
                    .lock()
                    .expect("Weechat notification sender lock is poisoned");

                let mut queue = q
                    .lock()
                    .expect("Lock of the future queue of the Weechat executor is poisoned");

                queue.push_back(ExecutorJob::BufferJob(BufferJob(
                    runnable,
                    buffer_name.clone(),
                )));
                weechat_notify
                    .send(())
                    .expect("Can't notify Weechat to run a future");
            }
        };

        let (runnable, task) = async_task::spawn_local(future, schedule);

        runnable.schedule();

        task
    }
}
