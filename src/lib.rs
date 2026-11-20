//! Find circular imports between files and workspace packages in JavaScript and TypeScript projects.
//!
//! `jscycles` is both a command-line tool and a library. To embed the whole
//! analysis, the same one the command runs, use [`Scan`]:
//!
//! ```no_run
//! use jscycles::{ColorMode, JscyclesError, Scan};
//!
//! # fn main() -> Result<(), JscyclesError> {
//! let report = Scan::new("path/to/repo").run()?;
//! if report.has_cycles() {
//!     std::io::Write::write_all(&mut std::io::stdout(), &report.to_human(ColorMode::Plain)?)?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! See the [`scan`] module for the options and results. The lower-level
//! modules ([`discovery`], [`extract`], [`graph`], [`cycles`]) are the parts a
//! scan is built from, for callers that supply their own import graph:
//!
//! ```
//! use std::path::PathBuf;
//!
//! use jscycles::cycles;
//! use jscycles::{DependencyGraph, Import, ImportTarget};
//!
//! let imports = vec![Import::new(
//!     PathBuf::from("a.ts"),
//!     ImportTarget::Resolved(PathBuf::from("a.ts")),
//!     "./a",
//! )];
//! let graph: DependencyGraph = imports.iter().collect();
//! let found = cycles::file_cycles(&graph);
//! assert_eq!(found.len(), 1);
//! ```

mod analysis;
mod input;
mod module_path;
mod module_syntax;
mod syntax;
mod terminal;

pub mod budget;
pub mod config;
pub mod cycles;
pub mod discovery;
pub mod extract;
pub mod graph;
pub mod import;
pub mod output;
pub mod scan;
pub mod tsconfig;
pub mod workspace;

pub use config::{Config, ConfigError};
pub use cycles::{Cycle, PackageCycle, PackageCycleEdge, PackageCycleWithFiles};
pub use discovery::{Discovered, DiscoveryError, Package, PackageDiscovery};
pub use extract::ImportExtractor;
pub use graph::{DependencyGraph, EdgeEvidence, PackageDependencyGraph};
pub use import::{ExtractError, Import, ImportTarget};
pub use output::{
    ColorMode, CycleFilter, HumanFormatter, JsonFormatter, OutputFormatter, PackageResult, Results,
    UnifiedOutputFormatter, UnifiedResults,
};
pub use scan::{JscyclesError, Scan, ScanReport, TsConfigSource};
pub use tsconfig::{TsConfig, TsConfigCache, TsConfigError};
pub use workspace::{Workspace, WorkspaceError, WorkspaceFormat, WorkspaceYamlError};
