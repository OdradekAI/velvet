#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobId(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobSnapshot {
    pub id: JobId,
    pub kind: String,
    pub status: JobStatus,
    pub progress: Option<u8>,
}

pub trait JobSystem {
    fn submit(&mut self, kind: String) -> JobId;
    fn cancel(&mut self, id: &JobId) -> bool;
    fn snapshot(&self, id: &JobId) -> Option<JobSnapshot>;
}
