//! Configuration file parsing for jscycles.
//!
//! Handles loading and parsing `jscycles.yaml` configuration files.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;
use serde::de::Error;
use thiserror::Error;

use crate::budget;
use crate::input;

/// Default include patterns for package scanning.
fn default_include() -> Vec<String> {
    vec![".".to_owned()]
}

/// Default exclude patterns for package scanning.
fn default_exclude() -> Vec<String> {
    vec![
        "**/node_modules".to_owned(),
        "**/dist".to_owned(),
        "**/build".to_owned(),
    ]
}

/// Default file extensions to analyze.
fn default_extensions() -> Vec<String> {
    DEFAULT_EXTENSIONS
        .iter()
        .map(|&extension| extension.to_owned())
        .collect()
}

/// Root configuration for jscycles.
///
/// Parse one from YAML, or build it with functional record update syntax so
/// that fields added later keep their defaults:
///
/// ```
/// use jscycles::budget::AnalysisLimits;
/// use jscycles::config::Config;
///
/// let config = Config::parse("defaults:\n  extensions: [ts]\n")?;
/// assert_eq!(config.extensions_for_package("@app/web"), ["ts"]);
///
/// let config = Config {
///     limits: AnalysisLimits {
///         max_files: 10,
///         ..AnalysisLimits::default()
///     },
///     ..Config::default()
/// };
/// assert!(config.validate().is_ok());
/// # Ok::<(), jscycles::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Whole-scan admission and enumeration limits.
    #[serde(default)]
    pub limits: budget::AnalysisLimits,
    /// Scan configuration for package discovery.
    #[serde(default)]
    pub scan: ScanConfig,

    /// Default settings for all packages.
    #[serde(default)]
    pub defaults: DefaultsConfig,

    /// Package-specific overrides keyed by package name glob pattern.
    #[serde(default)]
    pub packages: HashMap<String, PackageConfig>,
}

/// Configuration for package scanning/discovery.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanConfig {
    /// Glob patterns for directories to include in scanning.
    #[serde(default = "default_include")]
    pub include: Vec<String>,

    /// Glob patterns for directories to exclude from scanning.
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

impl Default for ScanConfig {
    #[inline]
    fn default() -> Self {
        Self {
            include: default_include(),
            exclude: default_exclude(),
        }
    }
}

/// Default settings applied to all packages.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DefaultsConfig {
    /// File extensions to analyze.
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,

    /// Leave out imports erased from JavaScript output: `import type`,
    /// `export type`, type-position `import()`, and declaration files.
    #[serde(default)]
    pub skip_type_imports: bool,
}

impl Default for DefaultsConfig {
    #[inline]
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            skip_type_imports: false,
        }
    }
}

/// Package-specific configuration overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PackageConfig {
    /// Override file extensions for this package.
    #[serde(default, deserialize_with = "present")]
    pub extensions: Option<Vec<String>>,

    /// Glob patterns for files to ignore within this package.
    #[serde(default, deserialize_with = "present")]
    pub ignore: Option<Vec<String>>,

    /// Override `defaults.skip_type_imports` for this package.
    #[serde(default, deserialize_with = "present")]
    pub skip_type_imports: Option<bool>,
}

/// An omitted override is `None`; an explicit YAML `null` is an error rather
/// than a silent "not configured", consistent with the other settings.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)?.map_or_else(
        || {
            Err(Error::custom(
                "explicit null is not allowed; omit the key to use the default",
            ))
        },
        |value| Ok(Some(value)),
    )
}

/// [`Config::parse`].
impl FromStr for Config {
    type Err = ConfigError;

    fn from_str(contents: &str) -> Result<Self, Self::Err> {
        Self::parse(contents)
    }
}

impl Config {
    /// Load configuration from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::Read` if the file cannot be read,
    /// `ConfigError::Parse` if the YAML is invalid, or `ConfigError::Invalid`
    /// if settings violate their semantic contract.
    #[inline]
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = input::read_text(path).map_err(ConfigError::Read)?;
        Self::parse(&contents)
    }

    /// Parse bounded configuration text and validate all settings, including
    /// overrides for packages that are not selected in this scan.
    /// # Errors
    /// Rejects oversized text, malformed YAML, unknown/duplicate fields, invalid
    /// glob patterns, invalid extension spellings, and nonpositive quotas.
    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        if u64::try_from(contents.len()).unwrap_or(u64::MAX) > input::MAX_FILE_BYTES {
            return Err(ConfigError::Invalid(
                "configuration exceeds 16 MiB".to_owned(),
            ));
        }
        let config: Self = serde_saphyr::from_str(contents).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate settings built programmatically as well as deserialized inputs.
    /// # Errors
    /// Returns the invalid setting's semantic error.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let _validated = budget::AnalysisBudget::try_new(self.limits)
            .map_err(|error| ConfigError::Invalid(error.to_string()))?;
        let globs = self
            .scan
            .include
            .iter()
            .chain(&self.scan.exclude)
            .chain(self.packages.keys())
            .chain(
                self.packages
                    .values()
                    .flat_map(|package| package.ignore.iter().flatten()),
            );
        for pattern in globs {
            let _compiled = glob::Pattern::new(pattern).map_err(|error| {
                ConfigError::Invalid(format!("invalid glob {pattern:?}: {error}"))
            })?;
        }
        let extensions = self.defaults.extensions.iter().chain(
            self.packages
                .values()
                .flat_map(|package| package.extensions.iter().flatten()),
        );
        for extension in extensions {
            if extension.is_empty() || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return Err(ConfigError::Invalid(format!(
                    "extension {extension:?} must be a nonempty alphanumeric suffix without a leading dot"
                )));
            }
        }
        Ok(())
    }

    /// Load configuration, using defaults only when the file is absent.
    ///
    /// # Errors
    /// Returns read, size-limit, or parse errors for an existing file.
    pub fn try_load_or_default(path: &Path) -> Result<Self, ConfigError> {
        input::optional_metadata(path)
            .map_err(ConfigError::Read)?
            .map_or_else(|| Ok(Self::default()), |_| Self::load(path))
    }

    /// Get the effective extensions for a package, considering overrides.
    #[inline]
    #[must_use]
    pub fn extensions_for_package(&self, package_name: &str) -> &[String] {
        self.find_matching_package_config(package_name)
            .and_then(|package| package.extensions.as_deref())
            .unwrap_or(&self.defaults.extensions)
    }

    /// Get the ignore patterns for a package.
    #[inline]
    #[must_use]
    pub fn ignore_for_package(&self, package_name: &str) -> Option<&[String]> {
        self.find_matching_package_config(package_name)?
            .ignore
            .as_deref()
    }

    /// Find the first matching package config for a package name.
    #[cfg_attr(feature = "profiling", hotpath::measure)]
    fn find_matching_package_config(&self, package_name: &str) -> Option<&PackageConfig> {
        // Sort keys for deterministic iteration order
        let mut keys: Vec<_> = self.packages.keys().collect();
        keys.sort();
        for pattern in keys {
            if Self::pattern_matches(pattern, package_name) {
                return self.packages.get(pattern);
            }
        }
        None
    }

    /// Check if a glob pattern matches a package name.
    #[inline]
    fn pattern_matches(pattern: &str, package_name: &str) -> bool {
        glob::Pattern::new(pattern).is_ok_and(|p| p.matches(package_name))
    }
}

/// Source extensions analyzed when neither the CLI nor the configuration
/// selects others.
pub const DEFAULT_EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

/// Error type for configuration operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Configuration violates the declared setting grammar or quota contract.
    #[error("Invalid configuration: {0}")]
    Invalid(String),
    /// Failed to read config file.
    #[error("Failed to read config file: {0}")]
    Read(#[source] io::Error),

    /// Failed to parse config YAML. The message keeps the parser's first
    /// line (position and reason); its multi-line source excerpt would be
    /// escaped into one unreadable line by the CLI's control-character
    /// escaping. The full error remains available as the source.
    #[error("Failed to parse config YAML: {}", first_line(&.0.to_string()))]
    Parse(#[source] serde_saphyr::Error),
}

/// The first line of a multi-line message.
fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.scan.include, vec!["."]);
        assert_eq!(
            config.scan.exclude,
            vec!["**/node_modules", "**/dist", "**/build"]
        );
        assert_eq!(
            config.defaults.extensions,
            vec!["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
        );
    }

    #[test]
    fn yaml_errors_are_one_line_with_position_and_reason() {
        let error = Config::parse("scan: {inclde: [x]}").expect_err("unknown key is rejected");
        let message = error.to_string();
        assert!(!message.contains('\n'), "one line: {message:?}");
        assert!(
            message.contains("line 1 column 8") && message.contains("unknown field `inclde`"),
            "position and reason are kept: {message:?}"
        );
        assert!(
            Error::source(&error).is_some_and(|source| source.to_string().contains('\n')),
            "the full parser error stays available as the source"
        );
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
scan:
  include:
    - "libs/*"
    - "apps/*"
  exclude:
    - "**/node_modules"

defaults:
  extensions: [ts, tsx]

packages:
  "@myorg/legacy-*":
    extensions: [js]
"#;
        let config: Config =
            serde_saphyr::from_str(yaml).expect("test yaml should parse successfully");
        assert_eq!(config.scan.include, vec!["libs/*", "apps/*"]);
        assert_eq!(config.defaults.extensions, vec!["ts", "tsx"]);
        assert!(config.packages.contains_key("@myorg/legacy-*"));
    }
}
