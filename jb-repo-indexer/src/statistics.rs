use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub struct Statistics {
    pub successful_tasks: usize,
    pub problems: Vec<ProblemReport>,
    pub failures: Vec<ErrorReport>,
}

#[derive(Debug)]
pub struct StatisticsCollector {
    successful_tasks: usize,
    problems: Vec<ProblemReport>,
    failures: Vec<ErrorReport>,
    sender: UnboundedSender<TaskReport>,
    receiver: UnboundedReceiver<TaskReport>,
}

impl StatisticsCollector {
    /// Create a new statistics collector.
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        Self {
            successful_tasks: 0,
            problems: Vec::new(),
            failures: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Get a sender to send statistics.
    pub fn sender(&self) -> StatisticsSender {
        StatisticsSender {
            sender: self.sender.clone(),
        }
    }

    /// Run the collector.
    ///
    /// This never returns.
    pub async fn run(&mut self) {
        let mut buffer = Vec::new();

        loop {
            let received = self.receiver.recv_many(&mut buffer, 16).await;
            if received == 0 {
                unreachable!("we hold a sender while running");
            }

            for report in buffer.drain(..received) {
                match report.data {
                    TaskDataPoint::Succeeded => self.successful_tasks += 1,
                    TaskDataPoint::Failed(err) => {
                        tracing::error!("Task failed: {}: {}", report.name, err);

                        let mut src = err.source();
                        while let Some(err) = src {
                            tracing::error!("-> Caused by: {}", err);
                            src = err.source();
                        }

                        self.failures.push(ErrorReport {
                            task_name: report.name,
                            error: err,
                        })
                    },
                    TaskDataPoint::EncounteredProblem(err) => {
                        tracing::warn!("Task encountered a problem: {}: {}", report.name, err);

                        let mut src = err.source();
                        while let Some(err) = src {
                            tracing::error!("-> Caused by: {}", err);
                            src = err.source();
                        }

                        self.problems.push(ProblemReport {
                            task_name: report.name,
                            error: err,
                        })
                    },
                }
            }
        }
    }

    pub fn reset(&mut self) -> Statistics {
        let stats = Statistics {
            successful_tasks: self.successful_tasks,
            problems: std::mem::take(&mut self.problems),
            failures: std::mem::take(&mut self.failures),
        };

        self.successful_tasks = 0;

        stats
    }
}

#[derive(Debug)]
pub struct ProblemReport {
    pub task_name: String,
    pub error: Box<dyn std::error::Error + Send + 'static>,
}

#[derive(Debug)]
pub struct ErrorReport {
    pub task_name: String,
    pub error: Box<dyn std::error::Error + Send + 'static>,
}

#[derive(Debug)]
pub struct TaskReport {
    name: String,
    data: TaskDataPoint,
}

#[derive(Debug, Clone)]
pub struct StatisticsSender {
    sender: UnboundedSender<TaskReport>,
}

impl StatisticsSender {
    pub fn send_succeeded(&self, name: String) {
        let _ = self.sender.send(TaskReport {
            name,
            data: TaskDataPoint::Succeeded,
        });
    }

    pub fn send_failed(&self, name: String, error: Box<dyn std::error::Error + Send + 'static>) {
        let _ = self.sender.send(TaskReport {
            name,
            data: TaskDataPoint::Failed(error),
        });
    }

    pub fn send_problem(
        &self,
        name: impl Into<String>,
        error: Box<dyn std::error::Error + Send + 'static>,
    ) {
        let _ = self.sender.send(TaskReport {
            name: name.into(),
            data: TaskDataPoint::EncounteredProblem(error),
        });
    }

    pub fn guard_future<F, E>(
        &self,
        name: impl Into<String>,
        future: F,
    ) -> impl Future<Output = ()> + 'static
    where
        F: Future<Output = Result<(), E>> + 'static,
        E: std::error::Error + Send + 'static,
    {
        let name = name.into();
        let sender = self.sender.clone();

        async move {
            let _ = match future.await {
                Ok(()) => sender.send(TaskReport {
                    name,
                    data: TaskDataPoint::Succeeded,
                }),
                Err(err) => sender.send(TaskReport {
                    name,
                    data: TaskDataPoint::Failed(Box::new(err)),
                }),
            };
        }
    }
}

#[derive(Debug)]
enum TaskDataPoint {
    Succeeded,
    Failed(Box<dyn std::error::Error + Send + 'static>),
    EncounteredProblem(Box<dyn std::error::Error + Send + 'static>),
}
