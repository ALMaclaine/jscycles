//! Monotonic, shared admission budgets for one analysis operation.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use serde::Deserialize;

/// Units bounded across all stages and parallel workers in one scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    /// Files/directories visited, including excluded entries.
    Entries,
    /// Input files admitted for reading or queued for extraction.
    Files,
    /// Input bytes read after configuration bootstrap.
    Bytes,
    /// Extracted import records.
    Imports,
    /// Nodes admitted to analysis graphs.
    Nodes,
    /// Distinct edges admitted to analysis graphs.
    Edges,
    /// Materialized cycles.
    Cycles,
    /// Logical graph/iteration operations, including enumeration backtracking.
    Work,
    /// Reserved bytes for retained import/graph labels and evidence.
    GraphBytes,
    /// Reserved cycle-path and expanded-evidence bytes.
    CycleBytes,
    /// Encoded report bytes.
    OutputBytes,
    /// Largest proposed length of any single input file.
    FileBytes,
}

impl Resource {
    const fn code(self) -> u8 {
        match self {
            Self::Entries => 1,
            Self::Files => 2,
            Self::Bytes => 3,
            Self::Imports => 4,
            Self::Nodes => 5,
            Self::Edges => 6,
            Self::Cycles => 7,
            Self::Work => 8,
            Self::GraphBytes => 9,
            Self::CycleBytes => 10,
            Self::OutputBytes => 11,
            Self::FileBytes => 12,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Entries),
            2 => Some(Self::Files),
            3 => Some(Self::Bytes),
            4 => Some(Self::Imports),
            5 => Some(Self::Nodes),
            6 => Some(Self::Edges),
            7 => Some(Self::Cycles),
            8 => Some(Self::Work),
            9 => Some(Self::GraphBytes),
            10 => Some(Self::CycleBytes),
            11 => Some(Self::OutputBytes),
            12 => Some(Self::FileBytes),
            _ => None,
        }
    }
}

impl std::fmt::Display for Resource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Entries => "directory entries",
            Self::Files => "input files",
            Self::Bytes => "total input bytes",
            Self::Imports => "imports",
            Self::Nodes => "graph nodes",
            Self::Edges => "graph edges",
            Self::Cycles => "cycles",
            Self::Work => "analysis work",
            Self::GraphBytes => "retained graph/import bytes",
            Self::CycleBytes => "cycle/evidence bytes",
            Self::OutputBytes => "report bytes",
            Self::FileBytes => "bytes in one input file",
        })
    }
}

/// Finite per-scan quotas. Counts are cumulative admissions, not unique filenames
/// or a promise about process RSS. Zero is invalid; raising a quota is explicit.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisLimits {
    /// Maximum visited filesystem entries, including excluded entries.
    pub max_entries: u64,
    /// Maximum file admissions; repeated reads/scans consume quota again.
    pub max_files: u64,
    /// Maximum bytes read across source and metadata inputs.
    pub max_total_bytes: u64,
    /// Maximum retained import records.
    pub max_imports: u64,
    /// Maximum graph nodes across all file and package graphs.
    pub max_nodes: u64,
    /// Maximum distinct edges across those graphs.
    pub max_edges: u64,
    /// Maximum returned file and package cycles combined.
    pub max_cycles: u64,
    /// Maximum logical traversal/enumeration operations.
    pub max_work: u64,
    /// Maximum cumulative reservations for retained import/graph data.
    pub max_graph_bytes: u64,
    /// Maximum cumulative cycle-path and expanded-evidence reservations.
    pub max_cycle_bytes: u64,
    /// Maximum encoded stdout report size, staged before publication.
    pub max_output_bytes: u64,
    /// Maximum bytes in an individual metadata or source file.
    pub max_file_bytes: u64,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            max_files: 100_000,
            max_total_bytes: 512 * 1024 * 1024,
            max_imports: 2_000_000,
            max_nodes: 500_000,
            max_edges: 2_000_000,
            max_cycles: 10_000,
            max_work: 20_000_000,
            max_graph_bytes: 128 * 1024 * 1024,
            max_cycle_bytes: 32 * 1024 * 1024,
            max_output_bytes: 32 * 1024 * 1024,
            max_file_bytes: 16 * 1024 * 1024,
        }
    }
}

impl AnalysisLimits {
    /// Return the configured limit for one resource.
    #[must_use]
    pub const fn limit(self, resource: Resource) -> u64 {
        match resource {
            Resource::Entries => self.max_entries,
            Resource::Files => self.max_files,
            Resource::Bytes => self.max_total_bytes,
            Resource::Imports => self.max_imports,
            Resource::Nodes => self.max_nodes,
            Resource::Edges => self.max_edges,
            Resource::Cycles => self.max_cycles,
            Resource::Work => self.max_work,
            Resource::GraphBytes => self.max_graph_bytes,
            Resource::CycleBytes => self.max_cycle_bytes,
            Resource::OutputBytes => self.max_output_bytes,
            Resource::FileBytes => self.max_file_bytes,
        }
    }
}

/// Invalid configuration or an exhausted analysis quota. No partial result is success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitError {
    resource: Resource,
    limit: u64,
}

impl LimitError {
    /// Resource whose admission failed.
    #[must_use]
    pub const fn resource(self) -> Resource {
        self.resource
    }
    /// Configured maximum; zero denotes invalid configuration.
    #[must_use]
    pub const fn limit(self) -> u64 {
        self.limit
    }
}

impl std::fmt::Display for LimitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.limit == 0 {
            write!(
                formatter,
                "analysis limit for {} must be positive",
                self.resource
            )
        } else {
            write!(
                formatter,
                "analysis limit exceeded: {} (limit {})",
                self.resource, self.limit
            )
        }
    }
}
impl std::error::Error for LimitError {}

#[derive(Debug, Default)]
struct Counters {
    entries: AtomicU64,
    files: AtomicU64,
    bytes: AtomicU64,
    imports: AtomicU64,
    nodes: AtomicU64,
    edges: AtomicU64,
    cycles: AtomicU64,
    work: AtomicU64,
    graph_bytes: AtomicU64,
    cycle_bytes: AtomicU64,
    output_bytes: AtomicU64,
    file_bytes: AtomicU64,
}

impl Counters {
    const fn counter(&self, resource: Resource) -> &AtomicU64 {
        match resource {
            Resource::Entries => &self.entries,
            Resource::Files => &self.files,
            Resource::Bytes => &self.bytes,
            Resource::Imports => &self.imports,
            Resource::Nodes => &self.nodes,
            Resource::Edges => &self.edges,
            Resource::Cycles => &self.cycles,
            Resource::Work => &self.work,
            Resource::GraphBytes => &self.graph_bytes,
            Resource::CycleBytes => &self.cycle_bytes,
            Resource::OutputBytes => &self.output_bytes,
            Resource::FileBytes => &self.file_bytes,
        }
    }
}

#[derive(Debug)]
struct Shared {
    limits: AnalysisLimits,
    used: Counters,
    failed: AtomicU8,
}

/// Cloneable handle to one scan's counters. Clones share atomic admissions and a
/// sticky failure; no reservation is refunded. Use a new budget for a new scan.
#[derive(Debug, Clone)]
pub struct AnalysisBudget {
    shared: Arc<Shared>,
}

impl Default for AnalysisBudget {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared {
                limits: AnalysisLimits::default(),
                used: Counters::default(),
                failed: AtomicU8::new(0),
            }),
        }
    }
}

impl AnalysisBudget {
    /// Create a shared budget.
    /// # Errors
    /// Rejects any zero limit.
    pub fn new(limits: AnalysisLimits) -> Result<Self, LimitError> {
        for code in 1..=12 {
            if let Some(resource) = Resource::from_code(code)
                && limits.limit(resource) == 0
            {
                return Err(LimitError { resource, limit: 0 });
            }
        }
        Ok(Self {
            shared: Arc::new(Shared {
                limits,
                used: Counters::default(),
                failed: AtomicU8::new(0),
            }),
        })
    }

    /// The immutable limits selected when this budget was created.
    #[must_use]
    pub fn limits(&self) -> AnalysisLimits {
        self.shared.limits
    }

    /// Successful cumulative admissions (the maximum proposed size for `FileBytes`).
    #[must_use]
    pub fn used(&self, resource: Resource) -> u64 {
        self.shared.used.counter(resource).load(Ordering::Relaxed)
    }

    /// Reject a scan after any worker exceeded a quota, even if a best-effort
    /// metadata adapter discarded its immediate error.
    /// # Errors
    /// Returns the first recorded exhaustion.
    pub fn check(&self) -> Result<(), LimitError> {
        Resource::from_code(self.shared.failed.load(Ordering::Acquire)).map_or(Ok(()), |resource| {
            Err(LimitError {
                resource,
                limit: self.shared.limits.limit(resource),
            })
        })
    }

    /// Reserve units before retaining data or doing the corresponding work.
    /// `FileBytes` checks a proposed single-file length and tracks its maximum.
    /// # Errors
    /// Fails permanently on exhaustion or arithmetic overflow; concurrent workers
    /// cannot overcommit the counter.
    pub fn consume(&self, resource: Resource, amount: u64) -> Result<(), LimitError> {
        self.check()?;
        let limit = self.shared.limits.limit(resource);
        if resource == Resource::FileBytes {
            if amount > limit {
                let _previous = self.shared.failed.compare_exchange(
                    0,
                    resource.code(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            } else {
                let _previous = self
                    .shared
                    .used
                    .file_bytes
                    .fetch_max(amount, Ordering::Relaxed);
            }
            return self.check();
        }
        if self
            .shared
            .used
            .counter(resource)
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(amount).filter(|next| *next <= limit)
            })
            .is_err()
        {
            let _previous = self.shared.failed.compare_exchange(
                0,
                resource.code(),
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        self.check()
    }

    pub(crate) fn io(&self, resource: Resource, amount: u64) -> std::io::Result<()> {
        self.consume(resource, amount)
            .map_err(std::io::Error::other)
    }
}

/// Internal generic policy keeps existing infallible library APIs infallible,
/// while the same iterative implementation serves explicitly budgeted callers.
pub(crate) trait Meter {
    type Error;
    fn charge(&self, resource: Resource, amount: u64) -> Result<(), Self::Error>;
}

impl Meter for AnalysisBudget {
    type Error = LimitError;
    fn charge(&self, resource: Resource, amount: u64) -> Result<(), Self::Error> {
        self.consume(resource, amount)
    }
}

pub(crate) struct Unbounded;
impl Meter for Unbounded {
    type Error = Infallible;
    fn charge(&self, _resource: Resource, _amount: u64) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) fn units(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Reports are fully staged under a byte quota before touching stdout.
pub struct BoundedReport {
    budget: AnalysisBudget,
    bytes: Vec<u8>,
}

impl BoundedReport {
    /// Start an empty report sharing the scan budget.
    #[must_use]
    pub const fn new(budget: AnalysisBudget) -> Self {
        Self {
            budget,
            bytes: Vec::new(),
        }
    }
    /// Return the complete encoded report.
    /// # Errors
    /// Rejects any recorded quota exhaustion.
    pub fn finish(self) -> Result<Vec<u8>, LimitError> {
        self.budget.check()?;
        Ok(self.bytes)
    }
}

impl std::io::Write for BoundedReport {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.budget.io(Resource::OutputBytes, units(bytes.len()))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.budget.check().map_err(std::io::Error::other)
    }
}

/// Reserve logical comparisons before a non-fallible sort.
pub(crate) fn sorting<M: Meter>(count: usize, meter: &M) -> Result<(), M::Error> {
    if let Some(passes) = count.checked_ilog2() {
        for _ in 0..=passes {
            meter.charge(Resource::Work, units(count))?;
        }
    }
    Ok(())
}
