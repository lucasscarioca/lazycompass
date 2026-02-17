use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyAction {
    Quit,
    MoveDown,
    MoveUp,
    Back,
    Forward,
    GoTop,
    GoBottom,
    NextPage,
    PreviousPage,
    Insert,
    Edit,
    Delete,
    SaveQuery,
    SaveAggregation,
    RunSavedQuery,
    RunSavedAggregation,
    ClearApplied,
    ToggleHelp,
    AddConnection,
}

#[derive(Debug, Clone, Copy)]
struct KeyBinding {
    action: KeyAction,
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HintGroup {
    pub(crate) actions: &'static [KeyAction],
    pub(crate) label: &'static str,
}

const KEY_BINDINGS: &[KeyBinding] = &[
    KeyBinding {
        action: KeyAction::Quit,
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::MoveDown,
        code: KeyCode::Char('j'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::MoveUp,
        code: KeyCode::Char('k'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Back,
        code: KeyCode::Char('h'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Forward,
        code: KeyCode::Char('l'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Forward,
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::GoBottom,
        code: KeyCode::Char('G'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::GoBottom,
        code: KeyCode::Char('G'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::NextPage,
        code: KeyCode::PageDown,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::PreviousPage,
        code: KeyCode::PageUp,
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Insert,
        code: KeyCode::Char('i'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Edit,
        code: KeyCode::Char('e'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::Delete,
        code: KeyCode::Char('d'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveQuery,
        code: KeyCode::Char('Q'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveQuery,
        code: KeyCode::Char('Q'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::SaveAggregation,
        code: KeyCode::Char('A'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::SaveAggregation,
        code: KeyCode::Char('A'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::RunSavedQuery,
        code: KeyCode::Char('r'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::RunSavedAggregation,
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ClearApplied,
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
    },
    KeyBinding {
        action: KeyAction::ToggleHelp,
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::SHIFT,
    },
    KeyBinding {
        action: KeyAction::AddConnection,
        code: KeyCode::Char('n'),
        modifiers: KeyModifiers::NONE,
    },
];

const HINT_MOVE: &[KeyAction] = &[KeyAction::MoveDown, KeyAction::MoveUp];
const HINT_SCROLL: &[KeyAction] = &[KeyAction::MoveDown, KeyAction::MoveUp];
const HINT_FORWARD: &[KeyAction] = &[KeyAction::Forward];
const HINT_BACK: &[KeyAction] = &[KeyAction::Back];
const HINT_TOP_BOTTOM: &[KeyAction] = &[KeyAction::GoTop, KeyAction::GoBottom];
const HINT_PAGE: &[KeyAction] = &[KeyAction::PreviousPage, KeyAction::NextPage];
const HINT_EDITING: &[KeyAction] = &[KeyAction::Insert, KeyAction::Edit, KeyAction::Delete];
const HINT_EDIT_DELETE: &[KeyAction] = &[KeyAction::Edit, KeyAction::Delete];
const HINT_SAVE: &[KeyAction] = &[KeyAction::SaveQuery, KeyAction::SaveAggregation];
const HINT_RUN: &[KeyAction] = &[KeyAction::RunSavedQuery, KeyAction::RunSavedAggregation];
const HINT_HELP: &[KeyAction] = &[KeyAction::ToggleHelp];
const HINT_QUIT: &[KeyAction] = &[KeyAction::Quit];

const CONNECTION_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "enter",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: &[KeyAction::AddConnection],
        label: "new connection",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const DATABASE_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "enter",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const COLLECTION_HINTS: &[HintGroup] = DATABASE_HINTS;

const DOCUMENT_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "view",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_EDITING,
        label: "insert/edit/delete",
    },
    HintGroup {
        actions: HINT_SAVE,
        label: "save query/agg",
    },
    HintGroup {
        actions: HINT_RUN,
        label: "run saved",
    },
    HintGroup {
        actions: HINT_PAGE,
        label: "page",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVED_QUERY_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "run",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "cancel",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVED_AGGREGATION_HINTS: &[HintGroup] = SAVED_QUERY_HINTS;

const ADD_CONNECTION_SCOPE_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_MOVE,
        label: "move",
    },
    HintGroup {
        actions: HINT_FORWARD,
        label: "select",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "cancel",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];

const SAVE_SCOPE_HINTS: &[HintGroup] = ADD_CONNECTION_SCOPE_HINTS;

const DOCUMENT_VIEW_HINTS: &[HintGroup] = &[
    HintGroup {
        actions: HINT_SCROLL,
        label: "scroll",
    },
    HintGroup {
        actions: HINT_BACK,
        label: "back",
    },
    HintGroup {
        actions: HINT_EDIT_DELETE,
        label: "edit/delete",
    },
    HintGroup {
        actions: HINT_TOP_BOTTOM,
        label: "top/bottom",
    },
    HintGroup {
        actions: HINT_HELP,
        label: "help",
    },
    HintGroup {
        actions: HINT_QUIT,
        label: "quit",
    },
];
impl KeyBinding {
    fn matches(&self, key: KeyEvent) -> bool {
        self.code == key.code && self.modifiers == key.modifiers
    }
}

pub(crate) fn action_for_key(key: KeyEvent) -> Option<KeyAction> {
    KEY_BINDINGS
        .iter()
        .find(|binding| binding.matches(key))
        .map(|binding| binding.action)
}
pub(crate) fn hint_groups(screen: Screen) -> &'static [HintGroup] {
    match screen {
        Screen::Connections => CONNECTION_HINTS,
        Screen::Databases => DATABASE_HINTS,
        Screen::Collections => COLLECTION_HINTS,
        Screen::Documents => DOCUMENT_HINTS,
        Screen::DocumentView => DOCUMENT_VIEW_HINTS,
        Screen::SavedQuerySelect => SAVED_QUERY_HINTS,
        Screen::SavedAggregationSelect => SAVED_AGGREGATION_HINTS,
        Screen::SaveQueryScopeSelect => SAVE_SCOPE_HINTS,
        Screen::SaveAggregationScopeSelect => SAVE_SCOPE_HINTS,
        Screen::AddConnectionScopeSelect => ADD_CONNECTION_SCOPE_HINTS,
    }
}

fn action_keys(action: KeyAction) -> &'static [&'static str] {
    match action {
        KeyAction::Quit => &["q"],
        KeyAction::MoveDown => &["j"],
        KeyAction::MoveUp => &["k"],
        KeyAction::Back => &["h"],
        KeyAction::Forward => &["l", "Enter"],
        KeyAction::GoTop => &["gg"],
        KeyAction::GoBottom => &["G"],
        KeyAction::NextPage => &["PgDn"],
        KeyAction::PreviousPage => &["PgUp"],
        KeyAction::Insert => &["i"],
        KeyAction::Edit => &["e"],
        KeyAction::Delete => &["d"],
        KeyAction::SaveQuery => &["Q"],
        KeyAction::SaveAggregation => &["A"],
        KeyAction::RunSavedQuery => &["r"],
        KeyAction::RunSavedAggregation => &["a"],
        KeyAction::ClearApplied => &["c"],
        KeyAction::ToggleHelp => &["?"],
        KeyAction::AddConnection => &["n"],
    }
}

pub(crate) fn keys_for_actions(actions: &[KeyAction]) -> String {
    let mut keys = Vec::new();
    for action in actions {
        keys.extend_from_slice(action_keys(*action));
    }
    keys.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn key_bindings_are_unique() {
        let mut seen = HashSet::new();
        for binding in KEY_BINDINGS {
            let key = format!("{:?}:{:?}", binding.code, binding.modifiers);
            assert!(seen.insert(key), "duplicate key binding: {binding:?}");
        }
    }
}
