//! Versioned JSON messages. References are scoped to one session, window and frame.
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 64 * 1024;
pub const MAX_TEXT: usize = 16 * 1024;
pub const MAX_TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub token: String,
    pub id: u64,
    pub session: String,
    pub window: String,
    pub command: Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Hello,
    Snapshot,
    Click {
        reference: String,
    },
    Press {
        key: String,
    },
    Type {
        reference: String,
        text: String,
    },
    Wait {
        condition: Condition,
        timeout_ms: u64,
    },
    Screenshot,
}
impl Command {
    pub fn validate(&self) -> Result<(), Failure> {
        match self {
            Self::Type { text, .. } if text.len() > MAX_TEXT => {
                Err(Failure::new("limit_exceeded", "Text exceeds 16 KiB"))
            }
            Self::Press { key } if key.len() > 100 => {
                Err(Failure::new("invalid_key", "Key is too long"))
            }
            Self::Wait { timeout_ms, .. } if *timeout_ms > MAX_TIMEOUT_MS => {
                Err(Failure::new("limit_exceeded", "Wait exceeds 10 seconds"))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    Present { author_id: String },
    Absent { author_id: String },
    Value { author_id: String, equals: String },
    Name { author_id: String, equals: String },
    Focused { author_id: String },
    FrameAfter { frame: u64 },
}
impl Condition {
    pub fn matches(&self, snapshot: &Snapshot) -> bool {
        if let Self::FrameAfter { frame } = self {
            return snapshot.frame > *frame;
        }
        let author_id = match self {
            Self::Present { author_id }
            | Self::Absent { author_id }
            | Self::Value { author_id, .. }
            | Self::Name { author_id, .. }
            | Self::Focused { author_id } => author_id,
            Self::FrameAfter { .. } => unreachable!(),
        };
        let nodes: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|n| n.author_id.as_ref() == Some(author_id))
            .collect();
        if matches!(self, Self::Absent { .. }) {
            return nodes.is_empty();
        }
        let [node] = nodes.as_slice() else {
            return false;
        };
        match self {
            Self::Present { .. } => node.visible,
            Self::Value { equals, .. } => node.value.as_ref() == Some(equals),
            Self::Name { equals, .. } => node.name.as_ref() == Some(equals),
            Self::Focused { .. } => node.focused,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub session: String,
    pub window: String,
    pub title: String,
    pub frame: u64,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub nodes: Vec<Node>,
}
impl Snapshot {
    pub fn resolve(&self, reference: &str) -> Result<&Node, Failure> {
        self.nodes
            .iter()
            .find(|n| n.reference == reference)
            .ok_or_else(|| {
                Failure::new(
                    "stale_ref",
                    "Take a new snapshot; reference is not in this committed frame",
                )
            })
    }
    pub fn by_id(&self, id: &str) -> Result<&Node, Failure> {
        let mut nodes = self
            .nodes
            .iter()
            .filter(|n| n.author_id.as_deref() == Some(id));
        let node = nodes.next().ok_or_else(|| Failure::new("not_found", id))?;
        if nodes.next().is_some() {
            return Err(Failure::new("ambiguous_target", id));
        }
        Ok(node)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub reference: String,
    pub parent: Option<String>,
    pub author_id: Option<String>,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<[f32; 4]>,
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub selected: Option<bool>,
    pub checked: Option<bool>,
    pub clickable: bool,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub version: u32,
    pub id: u64,
    #[serde(flatten)]
    pub result: Reply,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Reply {
    Ok {
        output: Output,
    },
    Error {
        error: Failure,
        snapshot: Option<Snapshot>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Output {
    Hello {
        session: String,
        window: String,
        title: String,
        capabilities: Vec<String>,
    },
    Snapshot {
        snapshot: Snapshot,
    },
    Acted {
        path: String,
        snapshot: Snapshot,
    },
    Screenshot {
        path: String,
        frame: u64,
        window: String,
        width: u32,
        height: u32,
        scale: f32,
    },
}
impl Output {
    pub fn snapshot(&self) -> Option<&Snapshot> {
        match self {
            Self::Snapshot { snapshot } | Self::Acted { snapshot, .. } => Some(snapshot),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Failure {
    pub code: String,
    pub message: String,
}
impl Failure {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Failure {}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            session: "a".into(),
            window: "main".into(),
            title: "Test".into(),
            frame: 2,
            width: 800.,
            height: 600.,
            scale: 1.,
            nodes: vec![Node {
                reference: "@a/main/2/1".into(),
                parent: None,
                author_id: Some("input".into()),
                role: "TextInput".into(),
                name: Some("Title".into()),
                value: Some("café 👋".into()),
                bounds: Some([0., 0., 100., 20.]),
                visible: true,
                enabled: true,
                focused: true,
                selected: None,
                checked: None,
                clickable: false,
                editable: true,
            }],
        }
    }
    #[test]
    fn references_and_predicates_are_scoped_and_exact() {
        let mut snap = snapshot();
        assert!(snap.resolve("@a/main/2/1").is_ok());
        for reference in ["@b/main/2/1", "@a/other/2/1", "@a/main/1/1"] {
            assert_eq!(snap.resolve(reference).unwrap_err().code, "stale_ref");
        }
        let condition = Condition::Value {
            author_id: "input".into(),
            equals: "café 👋".into(),
        };
        assert!(condition.matches(&snap));
        assert!(!Condition::FrameAfter { frame: 2 }.matches(&snap));
        snap.nodes.push(snap.nodes[0].clone());
        assert_eq!(snap.by_id("input").unwrap_err().code, "ambiguous_target");
        assert!(!condition.matches(&snap));
        snap.nodes.clear();
        assert!(
            Condition::Absent {
                author_id: "input".into()
            }
            .matches(&snap)
        );
    }
    #[test]
    fn bounded_typed_protocol_round_trips_unicode() {
        let command = Command::Type {
            reference: "@a/main/2/1".into(),
            text: "café 👋\nline two".into(),
        };
        let json = serde_json::to_string(&command).unwrap();
        assert!(
            serde_json::from_str::<Command>(&json)
                .unwrap()
                .validate()
                .is_ok()
        );
        assert!(serde_json::from_str::<Command>(r#"{"op":"shell","command":"anything"}"#).is_err());
        assert!(
            Command::Type {
                reference: "x".into(),
                text: "x".repeat(MAX_TEXT + 1)
            }
            .validate()
            .is_err()
        );
        assert!(
            Command::Wait {
                condition: Condition::FrameAfter { frame: 0 },
                timeout_ms: MAX_TIMEOUT_MS + 1
            }
            .validate()
            .is_err()
        );
    }
}
