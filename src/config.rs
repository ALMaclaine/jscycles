//! Configuration file parsing for jscycles.
//!
//! Handles loading and parsing `jscycles.yaml` configuration files.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ConfigError;

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
    vec![
        "ts".to_owned(),
        "tsx".to_owned(),
        "js".to_owned(),
        "jsx".to_owned(),
    ]
}

/// Root configuration for jscycles.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Whole-scan admission and enumeration limits.
    #[serde(default)]
    pub limits: crate::budget::AnalysisLimits,
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultsConfig {
    /// File extensions to analyze.
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
}

impl Default for DefaultsConfig {
    #[inline]
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
        }
    }
}

/// Package-specific configuration overrides.
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PackageConfig {
    /// Override file extensions for this package.
    #[serde(default, deserialize_with = "present")]
    pub extensions: Option<Vec<String>>,

    /// Glob patterns for files to ignore within this package.
    #[serde(default, deserialize_with = "present")]
    pub ignore: Option<Vec<String>>,
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
            Err(serde::de::Error::custom(
                "explicit null is not allowed; omit the key to use the default",
            ))
        },
        |value| Ok(Some(value)),
    )
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
        let contents = crate::input::read_text(path).map_err(ConfigError::Read)?;
        Self::parse(&contents)
    }

    /// Parse bounded configuration text and validate all settings, including
    /// overrides for packages that are not selected in this scan.
    /// # Errors
    /// Rejects oversized text, malformed YAML, unknown/duplicate fields, invalid
    /// glob patterns, invalid extension spellings, and nonpositive quotas.
    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        if u64::try_from(contents.len()).unwrap_or(u64::MAX) > crate::input::MAX_FILE_BYTES {
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
        let _validated = crate::budget::AnalysisBudget::new(self.limits)
            .map_err(|error| ConfigError::Invalid(error.to_string()))?;
        for pattern in self
            .scan
            .include
            .iter()
            .chain(&self.scan.exclude)
            .chain(self.packages.keys())
            .chain(
                self.packages
                    .values()
                    .flat_map(|package| package.ignore.iter().flatten()),
            )
        {
            let _compiled = glob::Pattern::new(pattern).map_err(|error| {
                ConfigError::Invalid(format!("invalid glob {pattern:?}: {error}"))
            })?;
        }
        for extension in self.defaults.extensions.iter().chain(
            self.packages
                .values()
                .flat_map(|package| package.extensions.iter().flatten()),
        ) {
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
        crate::input::optional_metadata(path)
            .map_err(ConfigError::Read)?
            .map_or_else(|| Ok(Self::default()), |_| Self::load(path))
    }

    /// Legacy best-effort loader; defaults on absent, unreadable, or invalid files.
    ///
    /// Use [`Self::try_load_or_default`] when a failed configuration must be reported.
    #[inline]
    #[must_use]
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// Get the effective extensions for a package, considering overrides.
    #[inline]
    #[must_use]
    pub fn extensions_for_package(&self, package_name: &str) -> Vec<String> {
        self.find_matching_package_config(package_name)
            .and_then(|pkg_config| pkg_config.extensions.clone())
            .unwrap_or_else(|| self.defaults.extensions.clone())
    }

    /// Get the ignore patterns for a package.
    #[inline]
    #[must_use]
    pub fn ignore_for_package(&self, package_name: &str) -> Option<Vec<String>> {
        let pkg_config = self.find_matching_package_config(package_name)?;
        pkg_config.ignore.clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.scan.include, vec!["."]);
        assert_eq!(
            config.scan.exclude,
            vec!["**/node_modules", "**/dist", "**/build"]
        );
        assert_eq!(config.defaults.extensions, vec!["ts", "tsx", "js", "jsx"]);
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
