//! Declarative rules about what a scan contains: what must never be removed, what needs a second
//! look, and what is safe to clean up. Rules live in `policy/<os>.toml` and are matched against the
//! arena after a scan.

mod lookup;
mod matcher;
mod reason;
mod schema;

pub use lookup::{expand, find_node};
pub use matcher::{covering, PolicyContext, PolicyHits, Resolved, POLICY_FLAGS};
pub use schema::{Kind, Match, PolicyError, Rule, Safety, SUPPORTED_VERSION};

use scan_core::{Arena, ChildIndex, NodeId};

const MACOS: &str = include_str!("../policy/macos.toml");
const LINUX: &str = include_str!("../policy/linux.toml");
const WINDOWS: &str = include_str!("../policy/windows.toml");

#[derive(Debug, Default)]
pub struct Policy {
    rules: Vec<Rule>,
}

impl Policy {
    pub fn from_toml(src: &str) -> Result<Self, PolicyError> {
        Ok(Self { rules: schema::parse(src)? })
    }

    /// The policy bundled for the OS this binary was built for.
    pub fn for_current_os() -> Result<Self, PolicyError> {
        let src = if cfg!(target_os = "macos") {
            MACOS
        } else if cfg!(target_os = "windows") {
            WINDOWS
        } else {
            LINUX
        };
        Self::from_toml(src)
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn rule(&self, idx: usize) -> &Rule {
        &self.rules[idx]
    }

    /// A rule's reason for one node, with `{age}` and `{size}` filled in.
    pub fn reason_for(&self, idx: usize, arena: &Arena, id: NodeId, ctx: &PolicyContext) -> String {
        reason::fill(&self.rules[idx].reason, arena, id, ctx.now)
    }

    /// Bind the rules to one scan. Rules whose `path`, `parent` or `under` is not in the scan are dropped.
    pub fn resolve<'a>(&'a self, arena: &Arena, index: &'a ChildIndex, ctx: &PolicyContext) -> Resolved<'a> {
        Resolved::new(&self.rules, arena, index, ctx)
    }
}

#[cfg(test)]
mod tests;
