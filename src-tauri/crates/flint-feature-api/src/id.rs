use std::{error::Error, fmt, str::FromStr};

/// Validation failure for a stable namespaced identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StableIdError {
    Empty,
    TooFewSegments { minimum: usize },
    InvalidSegment { segment: String },
}

impl fmt::Display for StableIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "stable ID must not be empty"),
            Self::TooFewSegments { minimum } => {
                write!(
                    formatter,
                    "stable ID must contain at least {minimum} dot-separated segments"
                )
            }
            Self::InvalidSegment { segment } => {
                write!(formatter, "stable ID segment '{segment}' is invalid")
            }
        }
    }
}

impl Error for StableIdError {}

/// A permanent, lowercase, dot-separated feature identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(String);

impl FeatureId {
    pub fn new(value: impl Into<String>) -> Result<Self, StableIdError> {
        let value = value.into();
        validate_namespaced_id(&value, 2)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for FeatureId {
    type Err = StableIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl AsRef<str> for FeatureId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A backend contribution identifier that includes its owning feature prefix.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContributionId {
    value: String,
    owner: FeatureId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionIdError {
    Invalid(StableIdError),
    NotOwned {
        owner: FeatureId,
        contribution: String,
    },
}

impl fmt::Display for ContributionIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => write!(formatter, "{error}"),
            Self::NotOwned { owner, contribution } => write!(
                formatter,
                "contribution ID '{contribution}' is not prefixed by owning feature ID '{owner}'"
            ),
        }
    }
}

impl Error for ContributionIdError {}

impl ContributionId {
    pub fn new(
        owner: &FeatureId,
        value: impl Into<String>,
    ) -> Result<Self, ContributionIdError> {
        let value = value.into();
        validate_namespaced_id(&value, 2).map_err(ContributionIdError::Invalid)?;
        let prefix = format!("{owner}.");
        if !value.starts_with(&prefix) || value.len() == prefix.len() {
            return Err(ContributionIdError::NotOwned {
                owner: owner.clone(),
                contribution: value,
            });
        }
        Ok(Self {
            value,
            owner: owner.clone(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn owner(&self) -> &FeatureId {
        &self.owner
    }
}

impl fmt::Display for ContributionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

pub(crate) fn validate_namespaced_id(
    value: &str,
    minimum_segments: usize,
) -> Result<(), StableIdError> {
    if value.is_empty() {
        return Err(StableIdError::Empty);
    }
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < minimum_segments {
        return Err(StableIdError::TooFewSegments {
            minimum: minimum_segments,
        });
    }
    for segment in segments {
        if !is_valid_segment(segment) {
            return Err(StableIdError::InvalidSegment {
                segment: segment.to_string(),
            });
        }
    }
    Ok(())
}

fn is_valid_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_ids_require_two_or_more_valid_lowercase_segments() {
        assert_eq!(FeatureId::new("flint.mods").unwrap().as_str(), "flint.mods");
        assert_eq!(
            "flint.loader-provider".parse::<FeatureId>().unwrap().as_str(),
            "flint.loader-provider"
        );

        for invalid in [
            "",
            "flint",
            "Flint.mods",
            "flint..mods",
            "flint.mods_impl",
            "flint.mod/s",
            "flint.-mods",
        ] {
            assert!(FeatureId::new(invalid).is_err(), "accepted '{invalid}'");
        }
    }

    #[test]
    fn contribution_ids_require_the_exact_owning_feature_prefix() {
        let owner = FeatureId::new("flint.mods").unwrap();
        let contribution = ContributionId::new(&owner, "flint.mods.runtime").unwrap();
        assert_eq!(contribution.as_str(), "flint.mods.runtime");
        assert_eq!(contribution.owner(), &owner);

        for invalid in [
            "flint.mods",
            "flint.modservice.runtime",
            "flint.other.runtime",
            "Flint.mods.runtime",
        ] {
            assert!(
                ContributionId::new(&owner, invalid).is_err(),
                "accepted '{invalid}'"
            );
        }
    }
}