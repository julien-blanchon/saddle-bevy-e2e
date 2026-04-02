//! Scenario definition and builder DSL.

use crate::action::Action;

/// A named sequence of actions forming an E2E test scenario.
pub struct Scenario {
    /// Unique name used for output directory and identification.
    pub name: String,
    /// Human-readable description of what this scenario verifies.
    pub description: String,
    /// Ordered list of actions to execute frame-by-frame.
    pub actions: Vec<Action>,
}

/// Builder for constructing scenarios with a fluent API.
pub struct ScenarioBuilder {
    name: String,
    description: String,
    actions: Vec<Action>,
}

impl ScenarioBuilder {
    /// Start building a new scenario with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            actions: Vec::new(),
        }
    }

    /// Set the scenario description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Append a single action.
    pub fn then(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    /// Append multiple actions from an iterator.
    pub fn then_many(mut self, actions: impl IntoIterator<Item = Action>) -> Self {
        self.actions.extend(actions);
        self
    }

    /// Append a `Handoff` action — the scenario stops but the game keeps running.
    ///
    /// Use this as the last step to leave the game in a specific state for
    /// interactive debugging via BRP. Real-time is restored so the game runs
    /// at normal speed.
    pub fn handoff(self) -> Self {
        self.then(Action::Handoff)
    }

    /// Finalize and return the scenario.
    pub fn build(self) -> Scenario {
        Scenario {
            name: self.name,
            description: self.description,
            actions: self.actions,
        }
    }
}

impl Scenario {
    /// Convenience: start building a scenario with the given name.
    pub fn builder(name: impl Into<String>) -> ScenarioBuilder {
        ScenarioBuilder::new(name)
    }
}
