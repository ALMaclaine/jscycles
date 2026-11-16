//! Import records: what a source file imports, and how extraction failed.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// An extracted import statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Import {
    /// File containing the import.
    pub source: PathBuf,

    /// What's being imported.
    pub target: ImportTarget,

    /// Decoded JavaScript module specifier value, without surrounding quotes.
    pub specifier: String,
}

/// Classification of an import target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImportTarget {
    /// Successfully resolved to a file path.
    Resolved(PathBuf),

    /// External module (`node_modules`, not a workspace package).
    External(String),

    /// Workspace package import.
    WorkspacePackage {
        /// The package name (e.g., "@myorg/utils").
        package_name: String,
        /// Optional subpath (e.g., "helpers" from "@myorg/utils/helpers").
        subpath: Option<String>,
    },

    /// Could not resolve the import.
    Unresolved(String),
}

impl ImportTarget {
    /// The bytes of path and name text this target retains, as charged to an
    /// analysis budget's graph bytes.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        match self {
            Self::Resolved(path) => path.as_os_str().len(),
            Self::External(text) | Self::Unresolved(text) => text.len(),
            Self::WorkspacePackage {
                package_name,
                subpath,
            } => package_name
                .len()
                .saturating_add(subpath.as_ref().map_or(0, String::len)),
        }
    }
}

impl Import {
    /// An import of `specifier`, written in `source`, that resolves to `target`.
    #[must_use]
    pub fn new(source: PathBuf, target: ImportTarget, specifier: impl Into<String>) -> Self {
        Self {
            source,
            target,
            specifier: specifier.into(),
        }
    }
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
        source: io::Error,
    },
}
