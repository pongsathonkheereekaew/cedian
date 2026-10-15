//! The agent's model settings as Settings > Models edits them: the
//! `modelRoles` record (`role -> provider/model[:thinking]`) and
//! `defaultThinkingLevel`. OMP's config CLI takes the record whole, so a
//! change to one role writes all of them (ADR-0045 decision 3).

use serde_json::Value;
use std::collections::BTreeMap;

/// The roles the agent itself uses; Settings lists them even when unset.
pub const BUILT_IN_ROLES: &[&str] = &["default", "smol", "slow", "plan", "advisor"];

/// How much the model reasons before answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Thinking {
    /// The agent picks per request; valid only as the default level.
    Auto,
    Minimal,
    Low,
    Medium,
    High,
    Max,
}

impl Thinking {
    /// The levels Settings offers as the default, in order.
    pub const DEFAULT_CHOICES: &[Thinking] = &[
        Self::Auto,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Max,
    ];
    /// The levels a role can pin after its model.
    pub const ROLE_CHOICES: &[Thinking] = &[
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Max,
    ];
    /// The agent's default for `defaultThinkingLevel`.
    pub const AGENT_DEFAULT: Thinking = Self::High;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Max => "Max",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::DEFAULT_CHOICES
            .iter()
            .copied()
            .find(|level| level.as_str() == text)
    }
}

/// What one role maps to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleModel {
    /// `provider/model`.
    pub model: String,
    pub thinking: Option<Thinking>,
}

impl RoleModel {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            thinking: None,
        }
    }

    /// `provider/model[:level]`; a suffix that is not a level is part of the
    /// model id.
    pub fn parse(text: &str) -> Self {
        let split = text.rsplit_once(':').and_then(|(model, suffix)| {
            let level = Thinking::parse(suffix).filter(|level| *level != Thinking::Auto)?;
            Some(Self {
                model: model.to_string(),
                thinking: Some(level),
            })
        });
        split.unwrap_or_else(|| Self::new(text))
    }

    pub fn provider(&self) -> &str {
        self.model
            .split_once('/')
            .map_or(self.model.as_str(), |(provider, _)| provider)
    }

    fn to_value(&self) -> String {
        match self.thinking {
            Some(level) => format!("{}:{}", self.model, level.as_str()),
            None => self.model.clone(),
        }
    }
}

/// The `modelRoles` record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelRoles(BTreeMap<String, RoleModel>);

impl ModelRoles {
    /// The record as the agent's config reports it; anything but an object
    /// of strings reads as no roles.
    pub fn from_value(value: Option<&Value>) -> Self {
        let Some(Value::Object(map)) = value else {
            return Self::default();
        };
        Self(
            map.iter()
                .filter_map(|(role, model)| Some((role.clone(), RoleModel::parse(model.as_str()?))))
                .collect(),
        )
    }

    pub fn get(&self, role: &str) -> Option<&RoleModel> {
        self.0.get(role)
    }

    pub fn set(&mut self, role: &str, model: RoleModel) {
        self.0.insert(role.to_string(), model);
    }

    pub fn remove(&mut self, role: &str) {
        self.0.remove(role);
    }

    /// The built-in roles, then every other role set, by name.
    pub fn rows(&self) -> Vec<String> {
        let mut rows: Vec<String> = BUILT_IN_ROLES.iter().map(|role| role.to_string()).collect();
        rows.extend(
            self.0
                .keys()
                .filter(|role| !BUILT_IN_ROLES.contains(&role.as_str()))
                .cloned(),
        );
        rows
    }

    /// The value `omp config set modelRoles` takes.
    pub fn to_config_value(&self) -> String {
        let record: serde_json::Map<String, Value> = self
            .0
            .iter()
            .map(|(role, model)| (role.clone(), Value::String(model.to_value())))
            .collect();
        Value::Object(record).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_role_reads_its_thinking_suffix_and_writes_it_back() {
        let value = json!({
            "default": "anthropic/claude-opus-5:high",
            "smol": "openrouter/qwen:free",
            "review": "opencode-go/glm-5.3",
            "broken": 7,
        });
        let mut roles = ModelRoles::from_value(Some(&value));
        assert_eq!(
            roles.get("default"),
            Some(&RoleModel {
                model: "anthropic/claude-opus-5".into(),
                thinking: Some(Thinking::High)
            })
        );
        assert_eq!(
            roles.get("smol"),
            Some(&RoleModel::new("openrouter/qwen:free"))
        );
        assert_eq!(roles.get("broken"), None);
        assert_eq!(roles.get("default").unwrap().provider(), "anthropic");
        assert_eq!(
            roles.rows(),
            ["default", "smol", "slow", "plan", "advisor", "review"]
        );

        roles.set(
            "plan",
            RoleModel {
                model: "openai/gpt-6".into(),
                thinking: Some(Thinking::Max),
            },
        );
        roles.remove("review");
        let written: Value = serde_json::from_str(&roles.to_config_value()).unwrap();
        assert_eq!(
            written,
            json!({
                "default": "anthropic/claude-opus-5:high",
                "smol": "openrouter/qwen:free",
                "plan": "openai/gpt-6:max",
            })
        );
    }

    #[test]
    fn a_role_named_with_quotes_stays_one_json_key() {
        let mut roles = ModelRoles::default();
        roles.set("a\"b", RoleModel::new("x/y"));
        let written: Value = serde_json::from_str(&roles.to_config_value()).unwrap();
        assert_eq!(written, json!({"a\"b": "x/y"}));
    }

    #[test]
    fn auto_is_a_default_level_not_a_role_suffix() {
        assert_eq!(RoleModel::parse("x/y:auto"), RoleModel::new("x/y:auto"));
        assert_eq!(Thinking::parse("auto"), Some(Thinking::Auto));
        assert!(!Thinking::ROLE_CHOICES.contains(&Thinking::Auto));
        assert_eq!(Thinking::AGENT_DEFAULT, Thinking::High);
    }
}
