use std::sync::mpsc;
use std::thread;

pub trait Task {
    type Output: Send;
    fn run(&self) -> Option<Self::Output>;
}

pub struct WorkQueue<TaskType: 'static + Task + Send> {
    send_tasks: Option<spmc::Sender<TaskType>>, // Option because it will be set to None to close the queue
    recv_tasks: spmc::Receiver<TaskType>,
    //send_output: mpsc::Sender<TaskType::Output>, // not need in the struct: each worker will have its own clone.
    recv_output: mpsc::Receiver<TaskType::Output>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl<TaskType: 'static + Task + Send> WorkQueue<TaskType> {
    pub fn new(n_workers: usize) -> WorkQueue<TaskType> {
        // TODO: create the channels; start the worker threads; record their JoinHandles
        let (send_tasks, recv_tasks) = spmc::channel();
        let (send_output, recv_output) = mpsc::channel();
        let mut workers = Vec::new();

        for _ in 0..n_workers {
            let recv_tasks_clone = recv_tasks.clone();
            let send_output_clone = send_output.clone();

            let handle = thread::spawn(move || {
                WorkQueue::<TaskType>::run(recv_tasks_clone, send_output_clone);
            });

            workers.push(handle);
        }

        WorkQueue {
            send_tasks: Some(send_tasks),
            recv_tasks,
            recv_output,
            workers,
        }
    }

    fn run(recv_tasks: spmc::Receiver<TaskType>, send_output: mpsc::Sender<TaskType::Output>) {
        // TODO: the main logic for a worker thread
        loop {
            match recv_tasks.recv() {
                Ok(task) => {
                    if let Some(result) = task.run() {
                        send_output.send(result).unwrap();
                    }
                }
                Err(_) => break, // Sender has been destroyed, no more tasks to receive
            }
        }
    }

    pub fn enqueue(&mut self, t: TaskType) -> Result<(), spmc::SendError<TaskType>> {
        // TODO: send this task to a worker
        if let Some(mut send_tasks) = self.send_tasks.take() {
            match send_tasks.send(t) {
                Ok(_) => {
                    self.send_tasks = Some(send_tasks);
                    Ok(())
                }
                Err(err) => {
                    self.send_tasks = Some(send_tasks);
                    Err(err)
                }
            }
        } else {
            Err(spmc::SendError(t))
        }
    }

    // Helper methods that let you receive results in various ways
    pub fn iter(&mut self) -> mpsc::Iter<TaskType::Output> {
        self.recv_output.iter()
    }
    pub fn recv(&mut self) -> TaskType::Output {
        self.recv_output
            .recv()
            .expect("I have been shutdown incorrectly")
    }
    pub fn try_recv(&mut self) -> Result<TaskType::Output, mpsc::TryRecvError> {
        self.recv_output.try_recv()
    }
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<TaskType::Output, mpsc::RecvTimeoutError> {
        self.recv_output.recv_timeout(timeout)
    }

    pub fn shutdown(&mut self) {
        // TODO: destroy the spmc::Sender so everybody knows no more tasks are incoming;
        // drain any pending tasks in the queue; wait for each worker thread to finish.
        // HINT: Vec.drain(..)
        drop(self.send_tasks.take()); // Signal that no more tasks are incoming

        // Drain any pending tasks in the queue
        while let Ok(_) = self.recv_tasks.try_recv() {}

        // Wait for each worker thread to finish
        for worker in self.workers.drain(..) {
            worker.join().unwrap();
        }
    }
}

impl<TaskType: 'static + Task + Send> Drop for WorkQueue<TaskType> {
    fn drop(&mut self) {
        // "Finalisation in destructors" pattern: https://rust-unofficial.github.io/patterns/idioms/dtor-finally.html
        match self.send_tasks {
            None => {} // already shut down
            Some(_) => self.shutdown(),
        }
    }
}
