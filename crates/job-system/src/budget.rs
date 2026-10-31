use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const JOB_BUDGET_POLICY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobBudgetLimits {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub child_processes: u32,
    pub concurrency: u32,
    /// Soft budget. The scheduler may throttle or backpressure when it is
    /// exceeded, but admission does not fail solely because of this value.
    pub cpu_millis_per_minute: u64,
    /// Soft budget. Network capability and byte accounting enforce this
    /// value; the ledger does not pretend to own the OS network stack.
    pub network_bytes_per_minute: u64,
}

impl JobBudgetLimits {
    pub fn hard_unlimited() -> Self {
        Self {
            memory_bytes: u64::MAX,
            disk_bytes: u64::MAX,
            child_processes: u32::MAX,
            concurrency: u32::MAX,
            cpu_millis_per_minute: u64::MAX,
            network_bytes_per_minute: u64::MAX,
        }
    }

    fn hard_fits(&self, request: &JobBudgetRequest) -> Option<BudgetResource> {
        (request.memory_bytes > self.memory_bytes)
            .then_some(BudgetResource::Memory)
            .or_else(|| (request.disk_bytes > self.disk_bytes).then_some(BudgetResource::Disk))
            .or_else(|| {
                (request.child_processes > self.child_processes)
                    .then_some(BudgetResource::ChildProcesses)
            })
            .or_else(|| {
                (request.concurrency > self.concurrency).then_some(BudgetResource::Concurrency)
            })
    }
}

impl Default for JobBudgetLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 512 * 1024 * 1024,
            disk_bytes: 2 * 1024 * 1024 * 1024,
            child_processes: 8,
            concurrency: 16,
            cpu_millis_per_minute: 60_000,
            network_bytes_per_minute: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobBudgetPolicy {
    pub schema_version: u16,
    pub device: JobBudgetLimits,
    pub default_owner: JobBudgetLimits,
    pub per_job: JobBudgetLimits,
    #[serde(default)]
    pub owners: BTreeMap<String, JobBudgetLimits>,
}

impl Default for JobBudgetPolicy {
    fn default() -> Self {
        let device = JobBudgetLimits::default();
        let mut default_owner = device.clone();
        default_owner.memory_bytes /= 2;
        default_owner.disk_bytes /= 2;
        default_owner.child_processes /= 2;
        default_owner.concurrency = 8;
        let mut per_job = default_owner.clone();
        per_job.memory_bytes /= 2;
        per_job.disk_bytes /= 2;
        per_job.child_processes = 1;
        per_job.concurrency = 1;
        Self {
            schema_version: JOB_BUDGET_POLICY_SCHEMA_VERSION,
            device,
            default_owner,
            per_job,
            owners: BTreeMap::new(),
        }
    }
}

impl JobBudgetPolicy {
    pub fn unlimited() -> Self {
        let unlimited = JobBudgetLimits::hard_unlimited();
        Self {
            schema_version: JOB_BUDGET_POLICY_SCHEMA_VERSION,
            device: unlimited.clone(),
            default_owner: unlimited.clone(),
            per_job: unlimited,
            owners: BTreeMap::new(),
        }
    }

    fn owner_limits(&self, owner_id: &str) -> &JobBudgetLimits {
        self.owners.get(owner_id).unwrap_or(&self.default_owner)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobBudgetRequest {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub child_processes: u32,
    pub concurrency: u32,
    pub cpu_millis_per_minute: u64,
    pub network_bytes_per_minute: u64,
}

impl Default for JobBudgetRequest {
    fn default() -> Self {
        Self {
            memory_bytes: 0,
            disk_bytes: 0,
            child_processes: 0,
            concurrency: 1,
            cpu_millis_per_minute: 0,
            network_bytes_per_minute: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetResource {
    Memory,
    Disk,
    ChildProcesses,
    Concurrency,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JobBudgetUsage {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub child_processes: u32,
    pub concurrency: u32,
    pub cpu_millis_per_minute: u64,
    pub network_bytes_per_minute: u64,
}

impl JobBudgetUsage {
    fn add(&mut self, request: &JobBudgetRequest) {
        self.memory_bytes = self.memory_bytes.saturating_add(request.memory_bytes);
        self.disk_bytes = self.disk_bytes.saturating_add(request.disk_bytes);
        self.child_processes = self.child_processes.saturating_add(request.child_processes);
        self.concurrency = self.concurrency.saturating_add(request.concurrency);
        self.cpu_millis_per_minute = self
            .cpu_millis_per_minute
            .saturating_add(request.cpu_millis_per_minute);
        self.network_bytes_per_minute = self
            .network_bytes_per_minute
            .saturating_add(request.network_bytes_per_minute);
    }

    fn subtract(&mut self, request: &JobBudgetRequest) {
        self.memory_bytes = self.memory_bytes.saturating_sub(request.memory_bytes);
        self.disk_bytes = self.disk_bytes.saturating_sub(request.disk_bytes);
        self.child_processes = self.child_processes.saturating_sub(request.child_processes);
        self.concurrency = self.concurrency.saturating_sub(request.concurrency);
        self.cpu_millis_per_minute = self
            .cpu_millis_per_minute
            .saturating_sub(request.cpu_millis_per_minute);
        self.network_bytes_per_minute = self
            .network_bytes_per_minute
            .saturating_sub(request.network_bytes_per_minute);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetRejection {
    pub resource: BudgetResource,
    pub requested: u64,
    pub available: u64,
}

#[derive(Default)]
pub(crate) struct JobBudgetLedger {
    policy: JobBudgetPolicy,
    device_usage: JobBudgetUsage,
    owner_usage: BTreeMap<String, JobBudgetUsage>,
    reservations: BTreeMap<String, (String, JobBudgetRequest)>,
}

impl JobBudgetLedger {
    pub(crate) fn new(policy: JobBudgetPolicy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }

    pub(crate) fn reserve(
        &mut self,
        job_id: &str,
        owner_id: &str,
        request: &JobBudgetRequest,
    ) -> Result<(), BudgetRejection> {
        if let Some(resource) = self.policy.per_job.hard_fits(request) {
            return Err(rejection_for(resource, request, &self.policy.per_job));
        }
        let owner_limits = self.policy.owner_limits(owner_id);
        let owner_usage = self.owner_usage.get(owner_id).cloned().unwrap_or_default();
        if let Some(resource) = exceeds_usage(&owner_usage, owner_limits, request) {
            return Err(rejection_for(resource, request, owner_limits));
        }
        if let Some(resource) = exceeds_usage(&self.device_usage, &self.policy.device, request) {
            return Err(rejection_for(resource, request, &self.policy.device));
        }
        self.device_usage.add(request);
        self.owner_usage
            .entry(owner_id.to_owned())
            .or_default()
            .add(request);
        self.reservations
            .insert(job_id.to_owned(), (owner_id.to_owned(), request.clone()));
        Ok(())
    }

    pub(crate) fn release(&mut self, job_id: &str) {
        let Some((owner_id, request)) = self.reservations.remove(job_id) else {
            return;
        };
        self.device_usage.subtract(&request);
        if let Some(usage) = self.owner_usage.get_mut(&owner_id) {
            usage.subtract(&request);
            if usage == &JobBudgetUsage::default() {
                self.owner_usage.remove(&owner_id);
            }
        }
    }

    pub(crate) fn usage(&self) -> JobBudgetUsage {
        self.device_usage.clone()
    }
}

fn exceeds_usage(
    usage: &JobBudgetUsage,
    limits: &JobBudgetLimits,
    request: &JobBudgetRequest,
) -> Option<BudgetResource> {
    (usage.memory_bytes.saturating_add(request.memory_bytes) > limits.memory_bytes)
        .then_some(BudgetResource::Memory)
        .or_else(|| {
            (usage.disk_bytes.saturating_add(request.disk_bytes) > limits.disk_bytes)
                .then_some(BudgetResource::Disk)
        })
        .or_else(|| {
            (usage
                .child_processes
                .saturating_add(request.child_processes)
                > limits.child_processes)
                .then_some(BudgetResource::ChildProcesses)
        })
        .or_else(|| {
            (usage.concurrency.saturating_add(request.concurrency) > limits.concurrency)
                .then_some(BudgetResource::Concurrency)
        })
}

fn rejection_for(
    resource: BudgetResource,
    request: &JobBudgetRequest,
    limits: &JobBudgetLimits,
) -> BudgetRejection {
    let requested = match resource {
        BudgetResource::Memory => request.memory_bytes,
        BudgetResource::Disk => request.disk_bytes,
        BudgetResource::ChildProcesses => request.child_processes as u64,
        BudgetResource::Concurrency => request.concurrency as u64,
    };
    let available = match resource {
        BudgetResource::Memory => limits.memory_bytes,
        BudgetResource::Disk => limits.disk_bytes,
        BudgetResource::ChildProcesses => limits.child_processes as u64,
        BudgetResource::Concurrency => limits.concurrency as u64,
    };
    BudgetRejection {
        resource,
        requested,
        available,
    }
}
