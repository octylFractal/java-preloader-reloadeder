pub mod key;

use crate::error::ESResult;
use crate::java_version::key::VersionKey;
use crate::string::SplittingExt;
use derive_more::Display;
use error_stack::{Context, Report, ResultExt};
use serde::Deserialize;
use std::cmp::Ordering;
use std::fmt::Display;
use std::str::{FromStr, Split};

#[derive(Debug, Display)]
pub struct JavaVersionParsingError;

impl Context for JavaVersionParsingError {}

/// Represents a Java version. Parsing is not strict, and some invalid versions may be accepted.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum JavaVersion {
    /// Older versioning scheme.
    /// Omits `major` and requires it to always be `1`.
    OldScheme(OldScheme),
    /// https://openjdk.org/jeps/223 or https://openjdk.org/jeps/322 based version.
    NewScheme(NewScheme),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct OldScheme {
    minor: u32,
    patch: u32,
    update: u32,
    build: Option<u32>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NewScheme {
    feature: u32,
    interim: u32,
    update: u32,
    patch: u32,
    trailing: Vec<u32>,
    pre_release: PreRelease,
    build: Option<u32>,
    opt: Option<String>,
}

impl JavaVersion {
    /// Compare two Java versions. Certain [PartialEq::ne] elements may be [Ordering::Equal].
    /// For example, [Self::NewScheme] `opt` information is not considered in the comparison.
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Old scheme vs old scheme
            (
                JavaVersion::OldScheme(OldScheme {
                    minor: self_minor,
                    patch: self_patch,
                    update: self_update,
                    build: self_build,
                }),
                JavaVersion::OldScheme(OldScheme {
                    minor: other_minor,
                    patch: other_patch,
                    update: other_update,
                    build: other_build,
                }),
            ) => {
                if self_minor != other_minor {
                    self_minor.cmp(other_minor)
                } else if self_patch != other_patch {
                    self_patch.cmp(other_patch)
                } else if self_update != other_update {
                    self_update.cmp(other_update)
                } else {
                    self_build.cmp(other_build)
                }
            }
            // Old scheme vs new scheme (always less)
            (
                JavaVersion::OldScheme(OldScheme {
                    minor: self_minor, ..
                }),
                JavaVersion::NewScheme(NewScheme {
                    feature: other_feature,
                    ..
                }),
            ) => {
                assert!(
                    self_minor < other_feature,
                    "Newer version scheme should always have a higher major version"
                );
                Ordering::Less
            }
            // New scheme vs old scheme (always greater)
            (
                JavaVersion::NewScheme(NewScheme {
                    feature: self_feature,
                    ..
                }),
                JavaVersion::OldScheme(OldScheme {
                    minor: other_minor, ..
                }),
            ) => {
                assert!(
                    self_feature > other_minor,
                    "Newer version scheme should always have a higher major version"
                );
                Ordering::Greater
            }
            // New scheme vs new scheme
            (
                JavaVersion::NewScheme(NewScheme {
                    feature: self_feature,
                    interim: self_interim,
                    update: self_update,
                    patch: self_patch,
                    trailing: self_trailing,
                    pre_release: self_pre_release,
                    build: self_build,
                    opt: _,
                }),
                JavaVersion::NewScheme(NewScheme {
                    feature: other_feature,
                    interim: other_interim,
                    update: other_update,
                    patch: other_patch,
                    trailing: other_trailing,
                    pre_release: other_pre_release,
                    build: other_build,
                    opt: _,
                }),
            ) => {
                if self_feature != other_feature {
                    self_feature.cmp(other_feature)
                } else if self_interim != other_interim {
                    self_interim.cmp(other_interim)
                } else if self_update != other_update {
                    self_update.cmp(other_update)
                } else if self_patch != other_patch {
                    self_patch.cmp(other_patch)
                } else if self_trailing != other_trailing {
                    self_trailing.cmp(other_trailing)
                } else if self_pre_release != other_pre_release {
                    self_pre_release.cmp(other_pre_release)
                } else {
                    self_build.cmp(other_build)
                }
                // Ignore opt, as it has no defined ordering
            }
        }
    }
}

impl Display for JavaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaVersion::OldScheme(v) => v.fmt(f),
            JavaVersion::NewScheme(v) => v.fmt(f),
        }
    }
}

impl Display for OldScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "1.{}.{}", self.minor, self.patch)?;
        if self.update != 0 {
            write!(f, "_{}", self.update)?;
        }
        if let Some(build) = self.build {
            write!(f, "-b{:02}", build)?;
        }
        Ok(())
    }
}

impl Display for NewScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.feature)?;
        let has_trailing = !self.trailing.is_empty();
        let has_at_least_patch = self.patch != 0 || has_trailing;
        let has_at_least_update = self.update != 0 || has_at_least_patch;
        if self.interim != 0 || has_at_least_update {
            write!(f, ".{}", self.interim)?;
        }
        if self.update != 0 || has_at_least_patch {
            write!(f, ".{}", self.update)?;
        }
        if self.patch != 0 || has_trailing {
            write!(f, ".{}", self.patch)?;
        }
        for t in &self.trailing {
            write!(f, ".{}", t)?;
        }
        match &self.pre_release {
            PreRelease::Other(s) => write!(f, "-{}", s)?,
            PreRelease::Numeric(n) => write!(f, "-{}", n)?,
            PreRelease::None => {}
        }
        if let Some(build) = self.build {
            write!(f, "+{}", build)?;
        }
        if let Some(opt) = &self.opt {
            if self.pre_release == PreRelease::None && self.build.is_none() {
                write!(f, "+")?;
            }
            write!(f, "-{}", opt)?;
        }
        Ok(())
    }
}

impl FromStr for JavaVersion {
    type Err = Report<JavaVersionParsingError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let extra_data_sep_index = s.find(['+', '-', '_']);
        let (vnum, extra) = match extra_data_sep_index {
            Some(i) => {
                let (v, e) = s.split_at(i);
                (v, Some(e))
            }
            None => (s, None),
        };
        let mut dot_parts = vnum.split('.');
        let first = dot_parts
            .next()
            .expect("split always has at least one element");
        if first == "1" {
            old_scheme(dot_parts, extra)
        } else {
            new_scheme(first, dot_parts, extra)
        }
    }
}

fn parse_numeric_part(v: &str, name: &str) -> ESResult<u32, JavaVersionParsingError> {
    v.parse::<u32>()
        .change_context(JavaVersionParsingError)
        .attach_printable_lazy(|| format!("Failed to parse {}", name))
        .attach_printable_lazy(|| format!("value: {}", v))
}

fn old_scheme(
    mut dot_parts: Split<char>,
    extra: Option<&str>,
) -> Result<JavaVersion, Report<JavaVersionParsingError>> {
    let minor = parse_numeric_part(
        dot_parts.next().ok_or_else(|| {
            Report::new(JavaVersionParsingError).attach_printable("Missing minor version")
        })?,
        "minor",
    )?;
    let patch = parse_numeric_part(
        dot_parts.next().ok_or_else(|| {
            Report::new(JavaVersionParsingError).attach_printable("Missing patch version")
        })?,
        "patch",
    )?;
    if dot_parts.next().is_some() {
        return Err(Report::new(JavaVersionParsingError).attach_printable("Too many version parts"));
    }
    let (update, build) = parse_old_scheme_extra(extra)?;

    Ok(JavaVersion::OldScheme(OldScheme {
        minor,
        patch,
        update,
        build,
    }))
}

fn parse_old_scheme_extra(
    extra: Option<&str>,
) -> ESResult<(u32, Option<u32>), JavaVersionParsingError> {
    let Some(extra) = extra else {
        return Ok((0, None));
    };
    let (update, build) = if let Some(extra_no_under) = extra.strip_prefix('_') {
        // _UPDATE(-BUILD)?
        extra_no_under.split_optional('-')
    } else {
        // -BUILD
        ("0", Some(&extra[1..]))
    };

    Ok((
        parse_numeric_part(update, "update")?,
        build
            .map(|s| {
                if !s.starts_with('b') {
                    return Err(Report::new(JavaVersionParsingError)
                        .attach_printable("Build must start with 'b'"));
                }
                let numeric = s.strip_prefix("b0").unwrap_or(&s[1..]);
                parse_numeric_part(numeric, "build")
            })
            .transpose()?,
    ))
}

fn new_scheme(
    first: &str,
    dot_parts: Split<char>,
    extra: Option<&str>,
) -> Result<JavaVersion, Report<JavaVersionParsingError>> {
    if !first.chars().all(|c| c.is_ascii_digit()) {
        return Err(
            Report::new(JavaVersionParsingError).attach_printable("First part is not numeric")
        );
    }
    if first.starts_with('0') {
        return Err(
            Report::new(JavaVersionParsingError).attach_printable("First part cannot start with 0")
        );
    }
    let feature = parse_numeric_part(first, "feature")?;
    let remaining_dot_parts = dot_parts.collect::<Vec<_>>();
    let last_part = remaining_dot_parts.last();
    if last_part == Some(&"0") {
        return Err(Report::new(JavaVersionParsingError).attach_printable("Last part cannot be 0"));
    }

    fn parse_opt_vnum_part(v: Option<&str>, name: &str) -> ESResult<u32, JavaVersionParsingError> {
        v.map(|s| parse_numeric_part(s, name))
            .transpose()
            .map(|o| o.unwrap_or(0))
    }

    let interim = parse_opt_vnum_part(remaining_dot_parts.first().copied(), "interim")?;
    let update = parse_opt_vnum_part(remaining_dot_parts.get(1).copied(), "update")?;
    let patch = parse_opt_vnum_part(remaining_dot_parts.get(2).copied(), "patch")?;
    let trailing = remaining_dot_parts
        .iter()
        .skip(3)
        .map(|s| parse_numeric_part(s, "trailing"))
        .collect::<ESResult<Vec<_>, _>>()?;

    let (pre_release, build, opt) = parse_new_scheme_extra(extra)?;

    Ok(JavaVersion::NewScheme(NewScheme {
        feature,
        interim,
        update,
        patch,
        trailing,
        pre_release,
        build,
        opt,
    }))
}

type NewSchemeExtra = (PreRelease, Option<u32>, Option<String>);

fn parse_new_scheme_extra(
    extra: Option<&str>,
) -> ESResult<NewSchemeExtra, JavaVersionParsingError> {
    let Some(extra) = extra else {
        return Ok((PreRelease::None, None, None));
    };
    if let Some(extra_no_prefix) = extra.strip_prefix("+-") {
        // Only $OPT
        return Ok((PreRelease::None, None, Some(extra_no_prefix.to_string())));
    }
    let (pre_s, build_s, opt_s) = if extra.starts_with("+") {
        // +$BUILD(-$OPT)?
        let (build, opt) = extra.split_optional('-');
        (None, Some(build), opt)
    } else if let Some(extra_no_dash) = extra.strip_prefix('-') {
        // One of:
        // -$PRE+$BUILD(-$OPT)?
        // -$PRE(-$OPT)?
        let (pre_maybe_build, opt) = extra_no_dash.split_optional('-');
        let (pre, build) = pre_maybe_build.split_optional('+');
        (Some(pre), build, opt)
    } else {
        return Err(Report::new(JavaVersionParsingError)
            .attach_printable("Extra data must start with + or -"));
    };
    Ok((
        pre_s
            .map(PreRelease::from_str)
            .transpose()
            .expect("never fails")
            .unwrap_or(PreRelease::None),
        build_s
            .map(|s| parse_numeric_part(s, "build"))
            .transpose()?,
        opt_s.map(|s| s.to_string()),
    ))
}

impl<'de> Deserialize<'de> for JavaVersion {
    fn deserialize<D>(deserializer: D) -> Result<JavaVersion, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl From<JavaVersion> for VersionKey {
    fn from(value: JavaVersion) -> Self {
        match value {
            JavaVersion::OldScheme(OldScheme { minor, .. }) => VersionKey {
                major: minor,
                pre_release: PreRelease::None,
            },
            JavaVersion::NewScheme(NewScheme {
                feature,
                pre_release,
                ..
            }) => VersionKey {
                major: feature,
                pre_release,
            },
        }
    }
}

/// Pre-release information. Ordered Other < Numeric < None.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum PreRelease {
    Other(String),
    Numeric(u32),
    None,
}

impl FromStr for PreRelease {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().all(|c| c.is_ascii_digit()) && (s == "0" || !s.starts_with('0')) {
            Ok(s.parse::<u32>()
                .map(PreRelease::Numeric)
                .expect("numeric should always parse"))
        } else {
            Ok(PreRelease::Other(s.to_string()))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn assert_round_trip(v: &str, expected: JavaVersion) {
        let jv: JavaVersion = v.parse().unwrap();
        assert_eq!(expected, jv);
        assert_eq!(v, jv.to_string());
    }

    #[test]
    fn test_old_scheme() {
        assert_round_trip(
            "1.8.0",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 0,
                build: None,
            }),
        );
        assert_round_trip(
            "1.8.0-b01",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 0,
                build: Some(1),
            }),
        );
        assert_round_trip(
            "1.8.0_22",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 22,
                build: None,
            }),
        );
        assert_round_trip(
            "1.8.0_292-b01",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 292,
                build: Some(1),
            }),
        );
        assert_round_trip(
            "1.8.0_292-b10",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 292,
                build: Some(10),
            }),
        );
        assert_round_trip(
            "1.8.5-b100",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 5,
                update: 0,
                build: Some(100),
            }),
        );
        assert_round_trip(
            "1.8.0_2-b100",
            JavaVersion::OldScheme(OldScheme {
                minor: 8,
                patch: 0,
                update: 2,
                build: Some(100),
            }),
        );
    }

    #[test]
    fn test_new_scheme() {
        assert_round_trip(
            "9",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 0,
                update: 0,
                patch: 0,
                trailing: vec![],
                pre_release: PreRelease::None,
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9.1",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 1,
                update: 0,
                patch: 0,
                trailing: vec![],
                pre_release: PreRelease::None,
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9.0.1",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 0,
                update: 1,
                patch: 0,
                trailing: vec![],
                pre_release: PreRelease::None,
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9.1.0.4",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 1,
                update: 0,
                patch: 4,
                trailing: vec![],
                pre_release: PreRelease::None,
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9.0.0.0.5",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 0,
                update: 0,
                patch: 0,
                trailing: vec![5],
                pre_release: PreRelease::None,
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9.1.4-ea",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 1,
                update: 4,
                patch: 0,
                trailing: vec![],
                pre_release: PreRelease::Other("ea".to_string()),
                build: None,
                opt: None,
            }),
        );
        assert_round_trip(
            "9-ea+19",
            JavaVersion::NewScheme(NewScheme {
                feature: 9,
                interim: 0,
                update: 0,
                patch: 0,
                trailing: vec![],
                pre_release: PreRelease::Other("ea".to_string()),
                build: Some(19),
                opt: None,
            }),
        );
    }

    fn assert_compare_both_ways(a: &str, b: &str, expected: Ordering) {
        let a: JavaVersion = a.parse().unwrap();
        let b: JavaVersion = b.parse().unwrap();
        assert_eq!(expected, a.compare(&b));
        assert_eq!(expected.reverse(), b.compare(&a));
    }

    #[test]
    fn test_compare() {
        assert_compare_both_ways("1.7.0", "1.8.0", Ordering::Less);
        assert_compare_both_ways("1.8.0", "1.8.0", Ordering::Equal);
        assert_compare_both_ways("1.8.0", "1.8.1", Ordering::Less);
        assert_compare_both_ways("1.8.0", "9", Ordering::Less);
        assert_compare_both_ways("9", "9", Ordering::Equal);
        assert_compare_both_ways("9", "9.1", Ordering::Less);
        assert_compare_both_ways("9-ea", "9", Ordering::Less);
        assert_compare_both_ways("9-ea", "9-ea", Ordering::Equal);
        assert_compare_both_ways("9-ea", "9-ea+1", Ordering::Less);
        assert_compare_both_ways("9", "10", Ordering::Less);
    }
}
