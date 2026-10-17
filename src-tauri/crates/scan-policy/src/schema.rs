use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;

pub const SUPPORTED_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Protected,
    Risky,
    Cleanup,
    Unexpected,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Protected => "protected",
            Kind::Risky => "risky",
            Kind::Cleanup => "cleanup",
            Kind::Unexpected => "unexpected",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Safety {
    Safe,
    Review,
    Keep,
}

impl Safety {
    pub fn as_str(self) -> &'static str {
        match self {
            Safety::Safe => "safe",
            Safety::Review => "review",
            Safety::Keep => "keep",
        }
    }
}

/// Conditions a node must meet for a rule to apply. Every field that is set must hold.
/// `path`, `parent`, `under` and `not_path` are absolute, or start with `~` for the home folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Match {
    pub path: Option<String>,
    pub parent: Option<String>,
    pub under: Option<String>,
    pub name: Option<String>,
    pub suffix: Option<String>,
    pub prefix: Option<String>,
    #[serde(default)]
    pub not_named: Vec<String>,
    #[serde(default)]
    pub not_path: Vec<String>,
    pub dir: Option<bool>,
    pub sibling: Option<String>,
    pub min_bytes: Option<u64>,
    pub older_than_days: Option<u32>,
}

impl Match {
    pub fn is_empty(&self) -> bool {
        *self == Match::default()
    }

    fn paths(&self) -> impl Iterator<Item = (&'static str, &String)> {
        [("path", &self.path), ("parent", &self.parent), ("under", &self.under)]
            .into_iter()
            .filter_map(|(field, value)| value.as_ref().map(|v| (field, v)))
            .chain(self.not_path.iter().map(|v| ("not_path", v)))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub kind: Kind,
    pub safety: Option<Safety>,
    pub reason: String,
    #[serde(rename = "match")]
    pub matcher: Match,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PolicyError {
    Syntax(String),
    Version(u32),
    Rule { id: String, message: String },
    DuplicateId(String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::Syntax(msg) => write!(f, "policy: {msg}"),
            PolicyError::Version(v) => write!(f, "policy: unsupported version {v}, expected {SUPPORTED_VERSION}"),
            PolicyError::Rule { id, message } => write!(f, "policy rule `{id}`: {message}"),
            PolicyError::DuplicateId(id) => write!(f, "policy: duplicate rule id `{id}`"),
        }
    }
}

impl std::error::Error for PolicyError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    version: u32,
    #[serde(default)]
    rule: Vec<toml::Table>,
}

/// Parse and validate a policy file into its rules.
pub fn parse(src: &str) -> Result<Vec<Rule>, PolicyError> {
    let raw: RawPolicy = toml::from_str(src).map_err(|e| PolicyError::Syntax(e.to_string()))?;
    if raw.version != SUPPORTED_VERSION {
        return Err(PolicyError::Version(raw.version));
    }
    let mut seen = HashSet::new();
    let mut rules = Vec::with_capacity(raw.rule.len());
    for (i, table) in raw.rule.into_iter().enumerate() {
        let id = match table.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => format!("#{}", i + 1),
        };
        let fail = |message: String| PolicyError::Rule { id: id.clone(), message };
        let rule: Rule = toml::Value::Table(table).try_into().map_err(|e: toml::de::Error| fail(e.message().to_string()))?;
        validate(&rule).map_err(fail)?;
        if !seen.insert(rule.id.clone()) {
            return Err(PolicyError::DuplicateId(rule.id));
        }
        rules.push(rule);
    }
    Ok(rules)
}

fn validate(rule: &Rule) -> Result<(), String> {
    if rule.id.is_empty() {
        return Err("`id` must not be empty".into());
    }
    if rule.reason.trim().is_empty() {
        return Err("`reason` must not be empty".into());
    }
    if rule.safety.is_some() && rule.kind != Kind::Cleanup {
        return Err("`safety` only applies to kind = \"cleanup\"".into());
    }
    if rule.matcher.is_empty() {
        return Err("`match` needs at least one field".into());
    }
    for (field, value) in rule.matcher.paths() {
        if !(value.starts_with('/') || value == "~" || value.starts_with("~/")) {
            return Err(format!("`match.{field}` must be absolute or start with `~/`, got {value:?}"));
        }
    }
    Ok(())
}
