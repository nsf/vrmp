use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Action {
    None,
    Quit,
    ToggleUI,
    ResetWorldOrigin,
    Command(Vec<String>),
}

pub struct ActionBin {
    action: Option<Action>,
}

impl ActionBin {
    pub fn create() -> ActionBin {
        ActionBin { action: None }
    }

    pub fn put(&mut self, a: Action) {
        self.action = Some(a);
    }

    pub fn dispatch(&mut self) -> Option<Action> {
        std::mem::replace(&mut self.action, None)
    }
}
