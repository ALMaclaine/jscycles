//! Find circular imports between files and workspace packages in JavaScript and TypeScript projects.
//!
//! `jscycles` is a CLI tool that detects circular dependencies within JS/TS packages,
//! with file-level and workspace-package cycle detection.
//!
//! ```
//! use jscycles::{CycleDetector, DependencyGraph, Import, ImportTarget};
//! use std::path::PathBuf;
//!
//! let imports = vec![Import {
//!     source: PathBuf::from("a.ts"),
//!     target: ImportTarget::Resolved(PathBuf::from("a.ts")),
//!     specifier: "./a".into(),
//! }];
//! let cycles = CycleDetector::detect(&DependencyGraph::from_imports(&imports));
//! assert_eq!(cycles.len(), 1);
//! assert_eq!(cycles[0].path, vec![PathBuf::from("a.ts"), PathBuf::from("a.ts")]);
//! ```

mod analysis;
mod input;
mod module_syntax;
mod syntax;
mod terminal;

pub mod budget;

pub mod config;
pub mod cycles;
pub mod discovery;
pub mod graph;
pub mod imports;
pub mod output;
pub mod tsconfig;
pub mod workspace;

use std::path::PathBuf;

use thiserror::Error;

/// Top-level error type for jscycles operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JscyclesError {
    /// A whole-scan resource quota was exhausted or invalid.
    #[error(transparent)]
    Limit(#[from] budget::LimitError),
    /// Configuration file error.
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    /// Package discovery error.
    #[error("Discovery error: {0}")]
    Discovery(#[from] DiscoveryError),

    /// Import extraction error.
    #[error("Import extraction error: {0}")]
    Import(#[from] ExtractError),

    /// TypeScript config error.
    #[error("TSConfig error: {0}")]
    TsConfig(#[from] TsConfigError),

    /// Workspace detection error.
    #[error("Workspace error: {0}")]
    Workspace(#[from] WorkspaceError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Error type for configuration operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Configuration violates the declared setting grammar or quota contract.
    #[error("Invalid configuration: {0}")]
    Invalid(String),
    /// Failed to read config file.
    #[error("Failed to read config file: {0}")]
    Read(#[source] std::io::Error),

    /// Failed to parse config YAML.
    #[error("Failed to parse config YAML: {0}")]
    Parse(#[source] serde_saphyr::Error),
}

/// Error type for package discovery operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DiscoveryError {
    /// A selected package name identifies multiple physical directories.
    #[error(
        "duplicate package name {name:?} at {first} and {second}; exclude copies such as nested worktrees or fixtures with scan.exclude, or pass the intended directories explicitly"
    )]
    DuplicatePackage {
        /// The ambiguous package name.
        name: String,
        /// The first canonical directory.
        first: PathBuf,
        /// The other canonical directory.
        second: PathBuf,
    },
    /// Invalid or exhausted analysis limits.
    #[error(transparent)]
    Limit(#[from] budget::LimitError),
    /// Invalid glob pattern.
    #[error("Invalid glob pattern '{pattern}': {source}")]
    InvalidPattern {
        /// The invalid pattern.
        pattern: String,
        /// The underlying error.
        #[source]
        source: glob::PatternError,
    },

    /// Failed to read directory.
    #[error("Failed to read directory {path}: {source}")]
    ReadDir {
        /// The directory path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse package.json.
    #[error("Failed to parse package.json at {path}: {source}")]
    ParsePackageJson {
        /// The package.json path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: serde_json::Error,
    },

    /// Missing package name in package.json.
    #[error("Missing 'name' field in package.json at {0}")]
    MissingPackageName(PathBuf),
}

/// Error type for import extraction operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExtractError {
    /// A shared resource quota was exhausted.
    #[error(transparent)]
    Limit(#[from] budget::LimitError),
    /// Failed to parse file.
    #[error("Failed to parse {path}: {message}")]
    ParseError {
        /// The file path.
        path: PathBuf,
        /// The error message.
        message: String,
    },

    /// IO error during extraction.
    #[error("IO error reading {path}: {source}")]
    IoError {
        /// The file path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// Error type for TypeScript config operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TsConfigError {
    /// The actual selected or inherited input that caused a failure.
    #[error("TSConfig at {path}: {source}")]
    AtPath {
        /// Failing configuration path, including inherited inputs.
        path: PathBuf,
        /// Original typed cause.
        #[source]
        source: Box<Self>,
    },
    /// A shared analysis limit was exhausted.
    #[error(transparent)]
    Limit(#[from] budget::LimitError),
    /// Failed to read tsconfig.json.
    #[error("Failed to read tsconfig.json: {0}")]
    Read(#[source] std::io::Error),

    /// Failed to parse tsconfig.json.
    #[error("Failed to parse tsconfig.json: {0}")]
    Parse(#[source] serde_json::Error),

    /// Invalid path in tsconfig.
    #[error("Invalid path in tsconfig: {0}")]
    InvalidPath(String),

    /// Extended tsconfig not found.
    #[error("Extended tsconfig not found: {0}")]
    ExtendsNotFound(String),

    /// Extends chain too deep (possible circular reference).
    #[error("Extends chain too deep (max 10), possible circular reference")]
    ExtendsDepth,
}

impl TsConfigError {
    pub(crate) fn at_path(self, path: &std::path::Path) -> Self {
        match self {
            Self::AtPath { .. } | Self::Limit(_) => self,
            Self::Read(_)
            | Self::Parse(_)
            | Self::InvalidPath(_)
            | Self::ExtendsNotFound(_)
            | Self::ExtendsDepth => Self::AtPath {
                path: path.to_path_buf(),
                source: Box::new(self),
            },
        }
    }
}

/// Path and original cause of a YAML workspace parse failure.
/// Boxed by [`WorkspaceError::Yaml`] to keep the common Result representation small.
#[derive(Debug, Error)]
#[error("Failed to parse {path}: {source}")]
pub struct WorkspaceYamlError {
    /// The failing metadata path.
    pub path: PathBuf,
    /// Original YAML parser error.
    #[source]
    pub source: serde_saphyr::Error,
}

/// Error type for workspace detection operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WorkspaceError {
    /// JSON workspace metadata could not be parsed.
    #[error("Failed to parse {path}: {source}")]
    Json {
        /// The failing metadata path.
        path: PathBuf,
        /// Original JSON parser error.
        #[source]
        source: serde_json::Error,
    },
    /// YAML workspace metadata could not be parsed.
    #[error(transparent)]
    Yaml(Box<WorkspaceYamlError>),
    /// A shared analysis limit was exhausted.
    #[error(transparent)]
    Limit(#[from] budget::LimitError),
    /// Failed to read workspace config file.
    #[error("Failed to read {path}: {source}")]
    Read {
        /// The file path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Invalid workspace semantics or JSONC normalization.
    #[error("Failed to parse {path}: {message}")]
    Parse {
        /// The file path.
        path: PathBuf,
        /// The error message.
        message: String,
    },

    /// Invalid glob pattern.
    #[error("Invalid glob pattern '{pattern}': {source}")]
    InvalidPattern {
        /// The invalid pattern.
        pattern: String,
        /// The underlying error.
        #[source]
        source: glob::PatternError,
    },
}

// Re-exports for public API
/// Source extensions analyzed when neither the CLI nor the configuration
/// selects others.
pub const DEFAULT_EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

pub use config::Config;
pub use cycles::{
    Cycle, CycleDetector, PackageCycle, PackageCycleDetector, PackageCycleEdge,
    PackageCycleWithFiles,
};
pub use discovery::{Discovered, Package, PackageDiscovery};
pub use graph::{DependencyGraph, EdgeEvidence, PackageDependencyGraph};
pub use imports::{Import, ImportExtractor, ImportTarget};
pub use output::{
    CycleFilter, HumanFormatter, JsonFormatter, OutputFormatter, PackageResult, Results,
    UnifiedOutputFormatter, UnifiedResults,
};
pub use tsconfig::{TsConfig, TsConfigCache};
pub use workspace::{Workspace, WorkspaceFormat};
