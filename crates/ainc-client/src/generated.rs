#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};
#[allow(unused_imports)]
use progenitor_client::{ClientHooks, OperationInfo, RequestBuilderExt, encode_path};
/// Types used as operation parameters and responses.
#[allow(clippy::all)]
pub mod types {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`Acknowledgement`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "operation_id": {
    ///      "type": "string"
    ///    },
    ///    "result_id": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Acknowledgement {
        pub operation_id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_id: ::std::option::Option<i64>,
    }
    impl Acknowledgement {
        pub fn builder() -> builder::Acknowledgement {
            Default::default()
        }
    }
    ///Where an action is in its life.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Where an action is in its life.",
    ///  "type": "string",
    ///  "enum": [
    ///    "queued",
    ///    "running",
    ///    "completed",
    ///    "failed",
    ///    "superseded"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum ActionState {
        #[serde(rename = "queued")]
        Queued,
        #[serde(rename = "running")]
        Running,
        #[serde(rename = "completed")]
        Completed,
        #[serde(rename = "failed")]
        Failed,
        #[serde(rename = "superseded")]
        Superseded,
    }
    impl ::std::fmt::Display for ActionState {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Queued => f.write_str("queued"),
                Self::Running => f.write_str("running"),
                Self::Completed => f.write_str("completed"),
                Self::Failed => f.write_str("failed"),
                Self::Superseded => f.write_str("superseded"),
            }
        }
    }
    impl ::std::str::FromStr for ActionState {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "queued" => Ok(Self::Queued),
                "running" => Ok(Self::Running),
                "completed" => Ok(Self::Completed),
                "failed" => Ok(Self::Failed),
                "superseded" => Ok(Self::Superseded),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ActionState {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ActionState {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ActionState {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///One durable action as people see it.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "One durable action as people see it.",
    ///  "type": "object",
    ///  "required": [
    ///    "created_at",
    ///    "id",
    ///    "state",
    ///    "summary"
    ///  ],
    ///  "properties": {
    ///    "created_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "finished_at": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "state": {
    ///      "$ref": "#/components/schemas/ActionState"
    ///    },
    ///    "summary": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ActionView {
        pub created_at: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub finished_at: ::std::option::Option<i64>,
        pub id: ::std::string::String,
        pub state: ActionState,
        pub summary: ::std::string::String,
    }
    impl ActionView {
        pub fn builder() -> builder::ActionView {
            Default::default()
        }
    }
    ///`Assignee`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "kind",
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "kind": {
    ///      "$ref": "#/components/schemas/AssigneeKind"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Assignee {
        pub id: ::std::string::String,
        pub kind: AssigneeKind,
        pub name: ::std::string::String,
    }
    impl Assignee {
        pub fn builder() -> builder::Assignee {
            Default::default()
        }
    }
    ///`AssigneeKind`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "human",
    ///    "agent"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum AssigneeKind {
        #[serde(rename = "human")]
        Human,
        #[serde(rename = "agent")]
        Agent,
    }
    impl ::std::fmt::Display for AssigneeKind {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Human => f.write_str("human"),
                Self::Agent => f.write_str("agent"),
            }
        }
    }
    impl ::std::str::FromStr for AssigneeKind {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "human" => Ok(Self::Human),
                "agent" => Ok(Self::Agent),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for AssigneeKind {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for AssigneeKind {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for AssigneeKind {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Automation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agent_id",
    ///    "applied_revision",
    ///    "every_minutes",
    ///    "id",
    ///    "missed",
    ///    "name",
    ///    "overlap_skipped",
    ///    "paused",
    ///    "prompt",
    ///    "revision"
    ///  ],
    ///  "properties": {
    ///    "agent_id": {
    ///      "type": "string"
    ///    },
    ///    "applied_revision": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "every_minutes": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "missed": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "overlap_skipped": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "paused": {
    ///      "type": "boolean"
    ///    },
    ///    "prompt": {
    ///      "type": "string"
    ///    },
    ///    "revision": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Automation {
        pub agent_id: ::std::string::String,
        pub applied_revision: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        pub every_minutes: i64,
        pub id: ::std::string::String,
        pub missed: i64,
        pub name: ::std::string::String,
        pub overlap_skipped: i64,
        pub paused: bool,
        pub prompt: ::std::string::String,
        pub revision: i64,
    }
    impl Automation {
        pub fn builder() -> builder::Automation {
            Default::default()
        }
    }
    ///`AutomationCommand`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "every_minutes",
    ///        "kind",
    ///        "name",
    ///        "proposal"
    ///      ],
    ///      "properties": {
    ///        "every_minutes": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "save"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "proposal": {
    ///          "$ref": "#/components/schemas/TicketProposal"
    ///        },
    ///        "revision": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ],
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "paused",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "pause"
    ///          ]
    ///        },
    ///        "paused": {
    ///          "type": "boolean"
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "run_now"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum AutomationCommand {
        #[serde(rename = "save")]
        Save {
            every_minutes: i64,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            id: ::std::option::Option<::std::string::String>,
            name: ::std::string::String,
            proposal: TicketProposal,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            revision: ::std::option::Option<i64>,
        },
        #[serde(rename = "pause")]
        Pause {
            id: ::std::string::String,
            paused: bool,
            revision: i64,
        },
        #[serde(rename = "run_now")]
        RunNow {
            id: ::std::string::String,
            revision: i64,
        },
    }
    ///`AutomationReceipt`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct AutomationReceipt {
        pub result_id: ::std::string::String,
    }
    impl AutomationReceipt {
        pub fn builder() -> builder::AutomationReceipt {
            Default::default()
        }
    }
    ///`AutomationRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/AutomationCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct AutomationRequest {
        pub command: AutomationCommand,
        pub operation_id: ::std::string::String,
    }
    impl AutomationRequest {
        pub fn builder() -> builder::AutomationRequest {
            Default::default()
        }
    }
    ///`AutomationSnapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "history",
    ///    "occurrences",
    ///    "rules"
    ///  ],
    ///  "properties": {
    ///    "history": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/HistoryEntry"
    ///      }
    ///    },
    ///    "occurrences": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/OccurrenceView"
    ///      }
    ///    },
    ///    "rules": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Automation"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct AutomationSnapshot {
        pub history: ::std::vec::Vec<HistoryEntry>,
        pub occurrences: ::std::vec::Vec<OccurrenceView>,
        pub rules: ::std::vec::Vec<Automation>,
    }
    impl AutomationSnapshot {
        pub fn builder() -> builder::AutomationSnapshot {
            Default::default()
        }
    }
    ///`CalendarCommand`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "all_day",
    ///        "ends_at",
    ///        "kind",
    ///        "starts_at",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "all_day": {
    ///          "type": "boolean"
    ///        },
    ///        "ends_at": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create"
    ///          ]
    ///        },
    ///        "location": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "notes": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "starts_at": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "all_day",
    ///        "ends_at",
    ///        "id",
    ///        "kind",
    ///        "revision",
    ///        "starts_at",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "all_day": {
    ///          "type": "boolean"
    ///        },
    ///        "ends_at": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "update"
    ///          ]
    ///        },
    ///        "location": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "notes": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "starts_at": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "delete"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum CalendarCommand {
        #[serde(rename = "create")]
        Create {
            all_day: bool,
            ends_at: i64,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            location: ::std::option::Option<::std::string::String>,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            notes: ::std::option::Option<::std::string::String>,
            starts_at: i64,
            title: ::std::string::String,
        },
        #[serde(rename = "update")]
        Update {
            all_day: bool,
            ends_at: i64,
            id: ::std::string::String,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            location: ::std::option::Option<::std::string::String>,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            notes: ::std::option::Option<::std::string::String>,
            revision: i64,
            starts_at: i64,
            title: ::std::string::String,
        },
        #[serde(rename = "delete")]
        Delete {
            id: ::std::string::String,
            revision: i64,
        },
    }
    /*Times are Unix seconds; `ends_at` is exclusive, so an all-day event runs from
    one local midnight to the next.*/
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Times are Unix seconds; `ends_at` is exclusive, so an all-day event runs from\none local midnight to the next.",
    ///  "type": "object",
    ///  "required": [
    ///    "all_day",
    ///    "calendar",
    ///    "ends_at",
    ///    "id",
    ///    "revision",
    ///    "source",
    ///    "starts_at",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "all_day": {
    ///      "type": "boolean"
    ///    },
    ///    "calendar": {
    ///      "type": "string"
    ///    },
    ///    "color": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "ends_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "location": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "notes": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "revision": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "source": {
    ///      "$ref": "#/components/schemas/EventSource"
    ///    },
    ///    "starts_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarEvent {
        pub all_day: bool,
        pub calendar: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub color: ::std::option::Option<::std::string::String>,
        pub ends_at: i64,
        pub id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub location: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notes: ::std::option::Option<::std::string::String>,
        pub revision: i64,
        pub source: EventSource,
        pub starts_at: i64,
        pub title: ::std::string::String,
    }
    impl CalendarEvent {
        pub fn builder() -> builder::CalendarEvent {
            Default::default()
        }
    }
    /*Everything the calendar store holds between `window_start` and
    `window_end`; mirrored events in that window that are absent are removed.*/
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Everything the calendar store holds between `window_start` and\n`window_end`; mirrored events in that window that are absent are removed.",
    ///  "type": "object",
    ///  "required": [
    ///    "events",
    ///    "window_end",
    ///    "window_start"
    ///  ],
    ///  "properties": {
    ///    "events": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ImportedEvent"
    ///      }
    ///    },
    ///    "window_end": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "window_start": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarImportRequest {
        pub events: ::std::vec::Vec<ImportedEvent>,
        pub window_end: i64,
        pub window_start: i64,
    }
    impl CalendarImportRequest {
        pub fn builder() -> builder::CalendarImportRequest {
            Default::default()
        }
    }
    ///`CalendarReceipt`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarReceipt {
        pub result_id: ::std::string::String,
    }
    impl CalendarReceipt {
        pub fn builder() -> builder::CalendarReceipt {
            Default::default()
        }
    }
    ///`CalendarRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/CalendarCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarRequest {
        pub command: CalendarCommand,
        pub operation_id: ::std::string::String,
    }
    impl CalendarRequest {
        pub fn builder() -> builder::CalendarRequest {
            Default::default()
        }
    }
    ///`CalendarSnapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "events"
    ///  ],
    ///  "properties": {
    ///    "events": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/CalendarEvent"
    ///      }
    ///    },
    ///    "last_import": {
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref": "#/components/schemas/ActionView"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarSnapshot {
        pub events: ::std::vec::Vec<CalendarEvent>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub last_import: ::std::option::Option<ActionView>,
    }
    impl CalendarSnapshot {
        pub fn builder() -> builder::CalendarSnapshot {
            Default::default()
        }
    }
    ///`ClimateMode`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "off",
    ///    "cool",
    ///    "heat",
    ///    "heat_cool"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum ClimateMode {
        #[serde(rename = "off")]
        Off,
        #[serde(rename = "cool")]
        Cool,
        #[serde(rename = "heat")]
        Heat,
        #[serde(rename = "heat_cool")]
        HeatCool,
    }
    impl ::std::fmt::Display for ClimateMode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Off => f.write_str("off"),
                Self::Cool => f.write_str("cool"),
                Self::Heat => f.write_str("heat"),
                Self::HeatCool => f.write_str("heat_cool"),
            }
        }
    }
    impl ::std::str::FromStr for ClimateMode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "off" => Ok(Self::Off),
                "cool" => Ok(Self::Cool),
                "heat" => Ok(Self::Heat),
                "heat_cool" => Ok(Self::HeatCool),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ClimateMode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ClimateMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ClimateMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Command`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create_conversation"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "rename_conversation"
    ///          ]
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "delete_conversation"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "select_conversation"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "conversation_id",
    ///        "kind",
    ///        "prompt"
    ///      ],
    ///      "properties": {
    ///        "conversation_id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "send"
    ///          ]
    ///        },
    ///        "prompt": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "retry"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create_todo"
    ///          ]
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "completed",
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "completed": {
    ///          "type": "boolean"
    ///        },
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "complete_todo"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "delete_todo"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "model"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "select_model"
    ///          ]
    ///        },
    ///        "model": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum Command {
        #[serde(rename = "create_conversation")]
        CreateConversation,
        #[serde(rename = "rename_conversation")]
        RenameConversation {
            id: i64,
            title: ::std::string::String,
        },
        #[serde(rename = "delete_conversation")]
        DeleteConversation { id: i64 },
        #[serde(rename = "select_conversation")]
        SelectConversation { id: i64 },
        #[serde(rename = "send")]
        Send {
            conversation_id: i64,
            prompt: ::std::string::String,
        },
        #[serde(rename = "retry")]
        Retry { id: i64 },
        #[serde(rename = "create_todo")]
        CreateTodo { title: ::std::string::String },
        #[serde(rename = "complete_todo")]
        CompleteTodo { completed: bool, id: i64 },
        #[serde(rename = "delete_todo")]
        DeleteTodo { id: i64 },
        #[serde(rename = "select_model")]
        SelectModel { model: ::std::string::String },
    }
    ///`CommandRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/Command"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CommandRequest {
        pub command: Command,
        pub operation_id: ::std::string::String,
    }
    impl CommandRequest {
        pub fn builder() -> builder::CommandRequest {
            Default::default()
        }
    }
    ///`Comment`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "author_id",
    ///    "body",
    ///    "created_at",
    ///    "id",
    ///    "ticket_id"
    ///  ],
    ///  "properties": {
    ///    "author_id": {
    ///      "type": "string"
    ///    },
    ///    "body": {
    ///      "type": "string"
    ///    },
    ///    "created_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "ticket_id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Comment {
        pub author_id: ::std::string::String,
        pub body: ::std::string::String,
        pub created_at: i64,
        pub id: i64,
        pub ticket_id: i64,
    }
    impl Comment {
        pub fn builder() -> builder::Comment {
            Default::default()
        }
    }
    ///`ConnectionStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "models",
    ///    "signing_in"
    ///  ],
    ///  "properties": {
    ///    "account": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "auth_url": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "models": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Model"
    ///      }
    ///    },
    ///    "signing_in": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ConnectionStatus {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub account: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub auth_url: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        pub models: ::std::vec::Vec<Model>,
        pub signing_in: bool,
    }
    impl ConnectionStatus {
        pub fn builder() -> builder::ConnectionStatus {
            Default::default()
        }
    }
    ///`Conversation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "snippet",
    ///    "title",
    ///    "updated",
    ///    "updated_at"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "snippet": {
    ///      "type": "string"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    },
    ///    "updated": {
    ///      "type": "string"
    ///    },
    ///    "updated_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Conversation {
        pub id: i64,
        pub snippet: ::std::string::String,
        pub title: ::std::string::String,
        pub updated: ::std::string::String,
        pub updated_at: i64,
    }
    impl Conversation {
        pub fn builder() -> builder::Conversation {
            Default::default()
        }
    }
    ///`CreateTerminalSession`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CreateTerminalSession {
        pub id: ::std::string::String,
    }
    impl CreateTerminalSession {
        pub fn builder() -> builder::CreateTerminalSession {
            Default::default()
        }
    }
    ///`ErrorBody`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ErrorBody {
        pub code: ::std::string::String,
        pub message: ::std::string::String,
    }
    impl ErrorBody {
        pub fn builder() -> builder::ErrorBody {
            Default::default()
        }
    }
    ///`EventSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "agentinc",
    ///    "macos"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum EventSource {
        #[serde(rename = "agentinc")]
        Agentinc,
        #[serde(rename = "macos")]
        Macos,
    }
    impl ::std::fmt::Display for EventSource {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Agentinc => f.write_str("agentinc"),
                Self::Macos => f.write_str("macos"),
            }
        }
    }
    impl ::std::str::FromStr for EventSource {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "agentinc" => Ok(Self::Agentinc),
                "macos" => Ok(Self::Macos),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for EventSource {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for EventSource {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for EventSource {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ExecutionPage`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "executions",
    ///    "ui_available"
    ///  ],
    ///  "properties": {
    ///    "executions": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ExecutionView"
    ///      }
    ///    },
    ///    "next_page": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "ui_available": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ExecutionPage {
        pub executions: ::std::vec::Vec<ExecutionView>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub next_page: ::std::option::Option<::std::string::String>,
        pub ui_available: bool,
    }
    impl ExecutionPage {
        pub fn builder() -> builder::ExecutionPage {
            Default::default()
        }
    }
    ///`ExecutionView`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "run_id",
    ///    "started_at",
    ///    "status",
    ///    "workflow_id",
    ///    "workflow_type"
    ///  ],
    ///  "properties": {
    ///    "closed_at": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    },
    ///    "run_id": {
    ///      "type": "string"
    ///    },
    ///    "started_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "status": {
    ///      "type": "string"
    ///    },
    ///    "url": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "workflow_id": {
    ///      "type": "string"
    ///    },
    ///    "workflow_type": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ExecutionView {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub closed_at: ::std::option::Option<i64>,
        pub run_id: ::std::string::String,
        pub started_at: i64,
        pub status: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url: ::std::option::Option<::std::string::String>,
        pub workflow_id: ::std::string::String,
        pub workflow_type: ::std::string::String,
    }
    impl ExecutionView {
        pub fn builder() -> builder::ExecutionView {
            Default::default()
        }
    }
    ///`Health`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "status"
    ///  ],
    ///  "properties": {
    ///    "status": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Health {
        pub status: ::std::string::String,
    }
    impl Health {
        pub fn builder() -> builder::Health {
            Default::default()
        }
    }
    ///`HistoryEntry`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "automation_id",
    ///    "count",
    ///    "id",
    ///    "kind",
    ///    "observed_at"
    ///  ],
    ///  "properties": {
    ///    "automation_id": {
    ///      "type": "string"
    ///    },
    ///    "count": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "kind": {
    ///      "type": "string"
    ///    },
    ///    "observed_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HistoryEntry {
        pub automation_id: ::std::string::String,
        pub count: i64,
        pub id: i64,
        pub kind: ::std::string::String,
        pub observed_at: i64,
    }
    impl HistoryEntry {
        pub fn builder() -> builder::HistoryEntry {
            Default::default()
        }
    }
    ///`HomeClimate`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "gap",
    ///    "max",
    ///    "min",
    ///    "mode",
    ///    "pending"
    ///  ],
    ///  "properties": {
    ///    "action": {
    ///      "description": "What the system is doing now, such as Cooling or Idle.",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "ambient": {
    ///      "description": "Indoor temperature in °F.",
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ],
    ///      "format": "double"
    ///    },
    ///    "gap": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "max": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "min": {
    ///      "description": "The setpoints the thermostat accepts, and the Auto gap.",
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "mode": {
    ///      "$ref": "#/components/schemas/ClimateMode"
    ///    },
    ///    "pending": {
    ///      "type": "boolean"
    ///    },
    ///    "target": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    },
    ///    "target_high": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    },
    ///    "target_low": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeClimate {
        ///What the system is doing now, such as Cooling or Idle.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub action: ::std::option::Option<::std::string::String>,
        ///Indoor temperature in °F.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ambient: ::std::option::Option<f64>,
        pub gap: i64,
        pub max: i64,
        ///The setpoints the thermostat accepts, and the Auto gap.
        pub min: i64,
        pub mode: ClimateMode,
        pub pending: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub target: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub target_high: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub target_low: ::std::option::Option<i64>,
    }
    impl HomeClimate {
        pub fn builder() -> builder::HomeClimate {
            Default::default()
        }
    }
    ///`HomeCommand`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "key",
    ///        "kind",
    ///        "on"
    ///      ],
    ///      "properties": {
    ///        "key": {
    ///          "$ref": "#/components/schemas/SwitchKey"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "switch"
    ///          ]
    ///        },
    ///        "on": {
    ///          "type": "boolean"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "mode"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "set_climate_mode"
    ///          ]
    ///        },
    ///        "mode": {
    ///          "$ref": "#/components/schemas/ClimateMode"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "target"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "set_climate_target"
    ///          ]
    ///        },
    ///        "target": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "high",
    ///        "kind",
    ///        "low"
    ///      ],
    ///      "properties": {
    ///        "high": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "set_climate_range"
    ///          ]
    ///        },
    ///        "low": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum HomeCommand {
        #[serde(rename = "switch")]
        Switch { key: SwitchKey, on: bool },
        #[serde(rename = "set_climate_mode")]
        SetClimateMode { mode: ClimateMode },
        #[serde(rename = "set_climate_target")]
        SetClimateTarget { target: i64 },
        #[serde(rename = "set_climate_range")]
        SetClimateRange { high: i64, low: i64 },
    }
    ///`HomeConnection`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "access_token",
    ///    "base_url"
    ///  ],
    ///  "properties": {
    ///    "access_token": {
    ///      "description": "A Cloudflare Access service token is stored in the Keychain.",
    ///      "type": "boolean"
    ///    },
    ///    "base_url": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeConnection {
        ///A Cloudflare Access service token is stored in the Keychain.
        pub access_token: bool,
        pub base_url: ::std::string::String,
    }
    impl HomeConnection {
        pub fn builder() -> builder::HomeConnection {
            Default::default()
        }
    }
    ///`HomeConnectionRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "base_url"
    ///  ],
    ///  "properties": {
    ///    "access_client_id": {
    ///      "description": "Cloudflare Access service token; omit both for an endpoint without Access.",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "access_client_secret": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_url": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeConnectionRequest {
        ///Cloudflare Access service token; omit both for an endpoint without Access.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub access_client_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub access_client_secret: ::std::option::Option<::std::string::String>,
        pub base_url: ::std::string::String,
    }
    impl HomeConnectionRequest {
        pub fn builder() -> builder::HomeConnectionRequest {
            Default::default()
        }
    }
    ///`HomeReceipt`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "description": "The durable action that applies the command.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeReceipt {
        ///The durable action that applies the command.
        pub result_id: ::std::string::String,
    }
    impl HomeReceipt {
        pub fn builder() -> builder::HomeReceipt {
            Default::default()
        }
    }
    ///`HomeRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/HomeCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeRequest {
        pub command: HomeCommand,
        pub operation_id: ::std::string::String,
    }
    impl HomeRequest {
        pub fn builder() -> builder::HomeRequest {
            Default::default()
        }
    }
    ///`HomeSnapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "actions",
    ///    "reachable",
    ///    "switches"
    ///  ],
    ///  "properties": {
    ///    "actions": {
    ///      "description": "Recent Smart Home actions, newest first.",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/ActionView"
    ///      }
    ///    },
    ///    "climate": {
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref": "#/components/schemas/HomeClimate"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    },
    ///    "connection": {
    ///      "oneOf": [
    ///        {
    ///          "type": "null"
    ///        },
    ///        {
    ///          "allOf": [
    ///            {
    ///              "$ref": "#/components/schemas/HomeConnection"
    ///            }
    ///          ]
    ///        }
    ///      ]
    ///    },
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "reachable": {
    ///      "type": "boolean"
    ///    },
    ///    "switches": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/HomeSwitch"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeSnapshot {
        ///Recent Smart Home actions, newest first.
        pub actions: ::std::vec::Vec<ActionView>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub climate: ::std::option::Option<HomeClimate>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub connection: ::std::option::Option<HomeConnection>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        pub reachable: bool,
        pub switches: ::std::vec::Vec<HomeSwitch>,
    }
    impl HomeSnapshot {
        pub fn builder() -> builder::HomeSnapshot {
            Default::default()
        }
    }
    ///`HomeSwitch`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "key",
    ///    "label",
    ///    "lit",
    ///    "members",
    ///    "on",
    ///    "pending",
    ///    "room",
    ///    "total"
    ///  ],
    ///  "properties": {
    ///    "key": {
    ///      "$ref": "#/components/schemas/SwitchKey"
    ///    },
    ///    "label": {
    ///      "type": "string"
    ///    },
    ///    "lit": {
    ///      "description": "How many of the lights it covers are confirmed on, out of `total`.",
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "members": {
    ///      "description": "The single switches a group covers; empty for a single switch.",
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/SwitchKey"
    ///      }
    ///    },
    ///    "on": {
    ///      "description": "Every light it covers is on.",
    ///      "type": "boolean"
    ///    },
    ///    "pending": {
    ///      "description": "A change is on its way to the lights.",
    ///      "type": "boolean"
    ///    },
    ///    "room": {
    ///      "type": "string"
    ///    },
    ///    "total": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct HomeSwitch {
        pub key: SwitchKey,
        pub label: ::std::string::String,
        ///How many of the lights it covers are confirmed on, out of `total`.
        pub lit: i64,
        ///The single switches a group covers; empty for a single switch.
        pub members: ::std::vec::Vec<SwitchKey>,
        ///Every light it covers is on.
        pub on: bool,
        ///A change is on its way to the lights.
        pub pending: bool,
        pub room: ::std::string::String,
        pub total: i64,
    }
    impl HomeSwitch {
        pub fn builder() -> builder::HomeSwitch {
            Default::default()
        }
    }
    /*One event read from the macOS calendar store. `external_id` is stable per
    occurrence, so repeating events import once per date.*/
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "One event read from the macOS calendar store. `external_id` is stable per\noccurrence, so repeating events import once per date.",
    ///  "type": "object",
    ///  "required": [
    ///    "all_day",
    ///    "calendar",
    ///    "ends_at",
    ///    "external_id",
    ///    "starts_at",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "all_day": {
    ///      "type": "boolean"
    ///    },
    ///    "calendar": {
    ///      "type": "string"
    ///    },
    ///    "color": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "ends_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "external_id": {
    ///      "type": "string"
    ///    },
    ///    "location": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "notes": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "starts_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct ImportedEvent {
        pub all_day: bool,
        pub calendar: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub color: ::std::option::Option<::std::string::String>,
        pub ends_at: i64,
        pub external_id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub location: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notes: ::std::option::Option<::std::string::String>,
        pub starts_at: i64,
        pub title: ::std::string::String,
    }
    impl ImportedEvent {
        pub fn builder() -> builder::ImportedEvent {
            Default::default()
        }
    }
    ///`Model`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Model {
        pub id: ::std::string::String,
        pub name: ::std::string::String,
    }
    impl Model {
        pub fn builder() -> builder::Model {
            Default::default()
        }
    }
    ///`OccurrenceView`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "automation_id",
    ///    "id",
    ///    "scheduled_at",
    ///    "state"
    ///  ],
    ///  "properties": {
    ///    "automation_id": {
    ///      "type": "string"
    ///    },
    ///    "detail": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "scheduled_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "state": {
    ///      "type": "string"
    ///    },
    ///    "ticket_id": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct OccurrenceView {
        pub automation_id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub detail: ::std::option::Option<::std::string::String>,
        pub id: ::std::string::String,
        pub scheduled_at: i64,
        pub state: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ticket_id: ::std::option::Option<i64>,
    }
    impl OccurrenceView {
        pub fn builder() -> builder::OccurrenceView {
            Default::default()
        }
    }
    ///`Settings`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "model": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "selected_conversation": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub model: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub selected_conversation: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for Settings {
        fn default() -> Self {
            Self {
                model: Default::default(),
                selected_conversation: Default::default(),
            }
        }
    }
    impl Settings {
        pub fn builder() -> builder::Settings {
            Default::default()
        }
    }
    ///`Snapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "conversations",
    ///    "settings",
    ///    "todos",
    ///    "turns"
    ///  ],
    ///  "properties": {
    ///    "conversations": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Conversation"
    ///      }
    ///    },
    ///    "settings": {
    ///      "$ref": "#/components/schemas/Settings"
    ///    },
    ///    "todos": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Todo"
    ///      }
    ///    },
    ///    "turns": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Turn"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Snapshot {
        pub conversations: ::std::vec::Vec<Conversation>,
        pub settings: Settings,
        pub todos: ::std::vec::Vec<Todo>,
        pub turns: ::std::vec::Vec<Turn>,
    }
    impl Snapshot {
        pub fn builder() -> builder::Snapshot {
            Default::default()
        }
    }
    ///The switches AgentInc exposes, each one control-center group.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The switches AgentInc exposes, each one control-center group.",
    ///  "type": "string",
    ///  "enum": [
    ///    "all",
    ///    "lamps",
    ///    "bedroom_lamps",
    ///    "living_room_lamps",
    ///    "kitchen_ceiling",
    ///    "under_cabinet"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum SwitchKey {
        #[serde(rename = "all")]
        All,
        #[serde(rename = "lamps")]
        Lamps,
        #[serde(rename = "bedroom_lamps")]
        BedroomLamps,
        #[serde(rename = "living_room_lamps")]
        LivingRoomLamps,
        #[serde(rename = "kitchen_ceiling")]
        KitchenCeiling,
        #[serde(rename = "under_cabinet")]
        UnderCabinet,
    }
    impl ::std::fmt::Display for SwitchKey {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::All => f.write_str("all"),
                Self::Lamps => f.write_str("lamps"),
                Self::BedroomLamps => f.write_str("bedroom_lamps"),
                Self::LivingRoomLamps => f.write_str("living_room_lamps"),
                Self::KitchenCeiling => f.write_str("kitchen_ceiling"),
                Self::UnderCabinet => f.write_str("under_cabinet"),
            }
        }
    }
    impl ::std::str::FromStr for SwitchKey {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "all" => Ok(Self::All),
                "lamps" => Ok(Self::Lamps),
                "bedroom_lamps" => Ok(Self::BedroomLamps),
                "living_room_lamps" => Ok(Self::LivingRoomLamps),
                "kitchen_ceiling" => Ok(Self::KitchenCeiling),
                "under_cabinet" => Ok(Self::UnderCabinet),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for SwitchKey {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for SwitchKey {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for SwitchKey {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`TerminalSession`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "state",
    ///    "workspace_id"
    ///  ],
    ///  "properties": {
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "state": {
    ///      "type": "string"
    ///    },
    ///    "workspace_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TerminalSession {
        pub id: ::std::string::String,
        pub state: ::std::string::String,
        pub workspace_id: ::std::string::String,
    }
    impl TerminalSession {
        pub fn builder() -> builder::TerminalSession {
            Default::default()
        }
    }
    ///`Ticket`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "assignee_id",
    ///    "assignee_kind",
    ///    "generation",
    ///    "id",
    ///    "revision",
    ///    "status",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "assignee_id": {
    ///      "type": "string"
    ///    },
    ///    "assignee_kind": {
    ///      "$ref": "#/components/schemas/AssigneeKind"
    ///    },
    ///    "generation": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "revision": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "status": {
    ///      "$ref": "#/components/schemas/TicketStatus"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Ticket {
        pub assignee_id: ::std::string::String,
        pub assignee_kind: AssigneeKind,
        pub generation: i64,
        pub id: i64,
        pub revision: i64,
        pub status: TicketStatus,
        pub title: ::std::string::String,
    }
    impl Ticket {
        pub fn builder() -> builder::Ticket {
            Default::default()
        }
    }
    ///`TicketCommand`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "proposal"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create_assigned"
    ///          ]
    ///        },
    ///        "proposal": {
    ///          "$ref": "#/components/schemas/TicketProposal"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create"
    ///          ]
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "delete"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "rename"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "title": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision",
    ///        "status"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "set_status"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "status": {
    ///          "$ref": "#/components/schemas/TicketStatus"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "assignee_id",
    ///        "assignee_kind",
    ///        "id",
    ///        "kind",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "assignee_id": {
    ///          "type": "string"
    ///        },
    ///        "assignee_kind": {
    ///          "$ref": "#/components/schemas/AssigneeKind"
    ///        },
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "assign"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "revision"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "cancel"
    ///          ]
    ///        },
    ///        "revision": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "body",
    ///        "kind",
    ///        "ticket_id"
    ///      ],
    ///      "properties": {
    ///        "body": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "add_comment"
    ///          ]
    ///        },
    ///        "ticket_id": {
    ///          "type": "integer",
    ///          "format": "int64"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "instructions",
    ///        "kind",
    ///        "model",
    ///        "name"
    ///      ],
    ///      "properties": {
    ///        "instructions": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "register_agent"
    ///          ]
    ///        },
    ///        "model": {
    ///          "type": "string"
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum TicketCommand {
        #[serde(rename = "create_assigned")]
        CreateAssigned { proposal: TicketProposal },
        #[serde(rename = "create")]
        Create { title: ::std::string::String },
        #[serde(rename = "delete")]
        Delete { id: i64, revision: i64 },
        #[serde(rename = "rename")]
        Rename {
            id: i64,
            revision: i64,
            title: ::std::string::String,
        },
        #[serde(rename = "set_status")]
        SetStatus {
            id: i64,
            revision: i64,
            status: TicketStatus,
        },
        #[serde(rename = "assign")]
        Assign {
            assignee_id: ::std::string::String,
            assignee_kind: AssigneeKind,
            id: i64,
            revision: i64,
        },
        #[serde(rename = "cancel")]
        Cancel { id: i64, revision: i64 },
        #[serde(rename = "add_comment")]
        AddComment {
            body: ::std::string::String,
            ticket_id: i64,
        },
        #[serde(rename = "register_agent")]
        RegisterAgent {
            instructions: ::std::string::String,
            model: ::std::string::String,
            name: ::std::string::String,
        },
    }
    ///`TicketCommandRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/TicketCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TicketCommandRequest {
        pub command: TicketCommand,
        pub operation_id: ::std::string::String,
    }
    impl TicketCommandRequest {
        pub fn builder() -> builder::TicketCommandRequest {
            Default::default()
        }
    }
    ///`TicketContract`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TicketContract {
        pub title: ::std::string::String,
    }
    impl TicketContract {
        pub fn builder() -> builder::TicketContract {
            Default::default()
        }
    }
    /*A bounded proposal to create one actionable Ticket for a registered agent.
    The command boundary authorizes and commits it with a durable receipt.*/
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "A bounded proposal to create one actionable Ticket for a registered agent.\nThe command boundary authorizes and commits it with a durable receipt.",
    ///  "type": "object",
    ///  "required": [
    ///    "agent_id",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "agent_id": {
    ///      "type": "string"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TicketProposal {
        pub agent_id: ::std::string::String,
        pub title: ::std::string::String,
    }
    impl TicketProposal {
        pub fn builder() -> builder::TicketProposal {
            Default::default()
        }
    }
    ///`TicketReceipt`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "operation_id": {
    ///      "type": "string"
    ///    },
    ///    "result_id": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ],
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TicketReceipt {
        pub operation_id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub result_id: ::std::option::Option<i64>,
    }
    impl TicketReceipt {
        pub fn builder() -> builder::TicketReceipt {
            Default::default()
        }
    }
    ///`TicketSnapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "assignees",
    ///    "comments",
    ///    "runs",
    ///    "tickets"
    ///  ],
    ///  "properties": {
    ///    "assignees": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Assignee"
    ///      }
    ///    },
    ///    "comments": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Comment"
    ///      }
    ///    },
    ///    "runs": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/WorkRun"
    ///      }
    ///    },
    ///    "tickets": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Ticket"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TicketSnapshot {
        pub assignees: ::std::vec::Vec<Assignee>,
        pub comments: ::std::vec::Vec<Comment>,
        pub runs: ::std::vec::Vec<WorkRun>,
        pub tickets: ::std::vec::Vec<Ticket>,
    }
    impl TicketSnapshot {
        pub fn builder() -> builder::TicketSnapshot {
            Default::default()
        }
    }
    ///`TicketStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "backlog",
    ///    "to_do",
    ///    "in_progress",
    ///    "done"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
        schemars::JsonSchema,
    )]
    pub enum TicketStatus {
        #[serde(rename = "backlog")]
        Backlog,
        #[serde(rename = "to_do")]
        ToDo,
        #[serde(rename = "in_progress")]
        InProgress,
        #[serde(rename = "done")]
        Done,
    }
    impl ::std::fmt::Display for TicketStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Backlog => f.write_str("backlog"),
                Self::ToDo => f.write_str("to_do"),
                Self::InProgress => f.write_str("in_progress"),
                Self::Done => f.write_str("done"),
            }
        }
    }
    impl ::std::str::FromStr for TicketStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "backlog" => Ok(Self::Backlog),
                "to_do" => Ok(Self::ToDo),
                "in_progress" => Ok(Self::InProgress),
                "done" => Ok(Self::Done),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for TicketStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for TicketStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for TicketStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Todo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "completed",
    ///    "id",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "completed": {
    ///      "type": "boolean"
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Todo {
        pub completed: bool,
        pub id: i64,
        pub title: ::std::string::String,
    }
    impl Todo {
        pub fn builder() -> builder::Todo {
            Default::default()
        }
    }
    ///`Turn`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "conversation_id",
    ///    "id",
    ///    "prompt",
    ///    "state"
    ///  ],
    ///  "properties": {
    ///    "conversation_id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "prompt": {
    ///      "type": "string"
    ///    },
    ///    "response": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "state": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Turn {
        pub conversation_id: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        pub id: i64,
        pub prompt: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub response: ::std::option::Option<::std::string::String>,
        pub state: ::std::string::String,
    }
    impl Turn {
        pub fn builder() -> builder::Turn {
            Default::default()
        }
    }
    ///`Version`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "api",
    ///    "product",
    ///    "version"
    ///  ],
    ///  "properties": {
    ///    "api": {
    ///      "type": "integer",
    ///      "format": "int32",
    ///      "minimum": 0.0
    ///    },
    ///    "product": {
    ///      "type": "string"
    ///    },
    ///    "version": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Version {
        pub api: i32,
        pub product: ::std::string::String,
        pub version: ::std::string::String,
    }
    impl Version {
        pub fn builder() -> builder::Version {
            Default::default()
        }
    }
    ///`WorkRun`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "generation",
    ///    "run_id",
    ///    "state",
    ///    "ticket_id"
    ///  ],
    ///  "properties": {
    ///    "error": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "generation": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "run_id": {
    ///      "type": "string"
    ///    },
    ///    "state": {
    ///      "type": "string"
    ///    },
    ///    "ticket_id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct WorkRun {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        pub generation: i64,
        pub run_id: ::std::string::String,
        pub state: ::std::string::String,
        pub ticket_id: i64,
    }
    impl WorkRun {
        pub fn builder() -> builder::WorkRun {
            Default::default()
        }
    }
    ///`Workspace`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "color": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "icon": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct Workspace {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub icon: ::std::option::Option<::std::string::String>,
        pub id: ::std::string::String,
        pub name: ::std::string::String,
    }
    impl Workspace {
        pub fn builder() -> builder::Workspace {
            Default::default()
        }
    }
    ///`WorkspaceCommand`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "kind",
    ///        "name"
    ///      ],
    ///      "properties": {
    ///        "color": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "icon": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "create"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind",
    ///        "name"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "rename"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "required": [
    ///        "id",
    ///        "kind"
    ///      ],
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "switch"
    ///          ]
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    #[serde(tag = "kind")]
    pub enum WorkspaceCommand {
        #[serde(rename = "create")]
        Create {
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            color: ::std::option::Option<::std::string::String>,
            #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
            icon: ::std::option::Option<::std::string::String>,
            name: ::std::string::String,
        },
        #[serde(rename = "rename")]
        Rename {
            id: ::std::string::String,
            name: ::std::string::String,
        },
        #[serde(rename = "switch")]
        Switch { id: ::std::string::String },
    }
    ///`WorkspaceReceipt`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct WorkspaceReceipt {
        pub result_id: ::std::string::String,
    }
    impl WorkspaceReceipt {
        pub fn builder() -> builder::WorkspaceReceipt {
            Default::default()
        }
    }
    ///`WorkspaceRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "command",
    ///    "operation_id"
    ///  ],
    ///  "properties": {
    ///    "command": {
    ///      "$ref": "#/components/schemas/WorkspaceCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct WorkspaceRequest {
        pub command: WorkspaceCommand,
        pub operation_id: ::std::string::String,
    }
    impl WorkspaceRequest {
        pub fn builder() -> builder::WorkspaceRequest {
            Default::default()
        }
    }
    ///`WorkspaceState`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "current_id",
    ///    "workspaces"
    ///  ],
    ///  "properties": {
    ///    "current_id": {
    ///      "type": "string"
    ///    },
    ///    "workspaces": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Workspace"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct WorkspaceState {
        pub current_id: ::std::string::String,
        pub workspaces: ::std::vec::Vec<Workspace>,
    }
    impl WorkspaceState {
        pub fn builder() -> builder::WorkspaceState {
            Default::default()
        }
    }
    /// Types for composing complex structures.
    pub mod builder {
        #[derive(Clone, Debug)]
        pub struct Acknowledgement {
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            result_id: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        }
        impl ::std::default::Default for Acknowledgement {
            fn default() -> Self {
                Self {
                    operation_id: Err("no value supplied for operation_id".to_string()),
                    result_id: Ok(Default::default()),
                }
            }
        }
        impl Acknowledgement {
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Acknowledgement> for super::Acknowledgement {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Acknowledgement,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    operation_id: value.operation_id?,
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::Acknowledgement> for Acknowledgement {
            fn from(value: super::Acknowledgement) -> Self {
                Self {
                    operation_id: Ok(value.operation_id),
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ActionView {
            created_at: ::std::result::Result<i64, ::std::string::String>,
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            finished_at: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            state: ::std::result::Result<super::ActionState, ::std::string::String>,
            summary: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for ActionView {
            fn default() -> Self {
                Self {
                    created_at: Err("no value supplied for created_at".to_string()),
                    error: Ok(Default::default()),
                    finished_at: Ok(Default::default()),
                    id: Err("no value supplied for id".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    summary: Err("no value supplied for summary".to_string()),
                }
            }
        }
        impl ActionView {
            pub fn created_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.created_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for created_at: {e}"));
                self
            }
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn finished_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.finished_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for finished_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::ActionState>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn summary<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.summary = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for summary: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ActionView> for super::ActionView {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ActionView,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    created_at: value.created_at?,
                    error: value.error?,
                    finished_at: value.finished_at?,
                    id: value.id?,
                    state: value.state?,
                    summary: value.summary?,
                })
            }
        }
        impl ::std::convert::From<super::ActionView> for ActionView {
            fn from(value: super::ActionView) -> Self {
                Self {
                    created_at: Ok(value.created_at),
                    error: Ok(value.error),
                    finished_at: Ok(value.finished_at),
                    id: Ok(value.id),
                    state: Ok(value.state),
                    summary: Ok(value.summary),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Assignee {
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            kind: ::std::result::Result<super::AssigneeKind, ::std::string::String>,
            name: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Assignee {
            fn default() -> Self {
                Self {
                    id: Err("no value supplied for id".to_string()),
                    kind: Err("no value supplied for kind".to_string()),
                    name: Err("no value supplied for name".to_string()),
                }
            }
        }
        impl Assignee {
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn kind<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::AssigneeKind>,
                T::Error: ::std::fmt::Display,
            {
                self.kind = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for kind: {e}"));
                self
            }
            pub fn name<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.name = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for name: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Assignee> for super::Assignee {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Assignee,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    id: value.id?,
                    kind: value.kind?,
                    name: value.name?,
                })
            }
        }
        impl ::std::convert::From<super::Assignee> for Assignee {
            fn from(value: super::Assignee) -> Self {
                Self {
                    id: Ok(value.id),
                    kind: Ok(value.kind),
                    name: Ok(value.name),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Automation {
            agent_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            applied_revision: ::std::result::Result<i64, ::std::string::String>,
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            every_minutes: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            missed: ::std::result::Result<i64, ::std::string::String>,
            name: ::std::result::Result<::std::string::String, ::std::string::String>,
            overlap_skipped: ::std::result::Result<i64, ::std::string::String>,
            paused: ::std::result::Result<bool, ::std::string::String>,
            prompt: ::std::result::Result<::std::string::String, ::std::string::String>,
            revision: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for Automation {
            fn default() -> Self {
                Self {
                    agent_id: Err("no value supplied for agent_id".to_string()),
                    applied_revision: Err("no value supplied for applied_revision".to_string()),
                    error: Ok(Default::default()),
                    every_minutes: Err("no value supplied for every_minutes".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    missed: Err("no value supplied for missed".to_string()),
                    name: Err("no value supplied for name".to_string()),
                    overlap_skipped: Err("no value supplied for overlap_skipped".to_string()),
                    paused: Err("no value supplied for paused".to_string()),
                    prompt: Err("no value supplied for prompt".to_string()),
                    revision: Err("no value supplied for revision".to_string()),
                }
            }
        }
        impl Automation {
            pub fn agent_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.agent_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for agent_id: {e}"));
                self
            }
            pub fn applied_revision<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.applied_revision = value.try_into().map_err(|e| {
                    format!("error converting supplied value for applied_revision: {e}")
                });
                self
            }
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn every_minutes<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.every_minutes = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for every_minutes: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn missed<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.missed = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for missed: {e}"));
                self
            }
            pub fn name<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.name = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for name: {e}"));
                self
            }
            pub fn overlap_skipped<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.overlap_skipped = value.try_into().map_err(|e| {
                    format!("error converting supplied value for overlap_skipped: {e}")
                });
                self
            }
            pub fn paused<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.paused = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for paused: {e}"));
                self
            }
            pub fn prompt<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.prompt = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for prompt: {e}"));
                self
            }
            pub fn revision<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.revision = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for revision: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Automation> for super::Automation {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Automation,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    agent_id: value.agent_id?,
                    applied_revision: value.applied_revision?,
                    error: value.error?,
                    every_minutes: value.every_minutes?,
                    id: value.id?,
                    missed: value.missed?,
                    name: value.name?,
                    overlap_skipped: value.overlap_skipped?,
                    paused: value.paused?,
                    prompt: value.prompt?,
                    revision: value.revision?,
                })
            }
        }
        impl ::std::convert::From<super::Automation> for Automation {
            fn from(value: super::Automation) -> Self {
                Self {
                    agent_id: Ok(value.agent_id),
                    applied_revision: Ok(value.applied_revision),
                    error: Ok(value.error),
                    every_minutes: Ok(value.every_minutes),
                    id: Ok(value.id),
                    missed: Ok(value.missed),
                    name: Ok(value.name),
                    overlap_skipped: Ok(value.overlap_skipped),
                    paused: Ok(value.paused),
                    prompt: Ok(value.prompt),
                    revision: Ok(value.revision),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct AutomationReceipt {
            result_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for AutomationReceipt {
            fn default() -> Self {
                Self {
                    result_id: Err("no value supplied for result_id".to_string()),
                }
            }
        }
        impl AutomationReceipt {
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<AutomationReceipt> for super::AutomationReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: AutomationReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::AutomationReceipt> for AutomationReceipt {
            fn from(value: super::AutomationReceipt) -> Self {
                Self {
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct AutomationRequest {
            command: ::std::result::Result<super::AutomationCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for AutomationRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl AutomationRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::AutomationCommand>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<AutomationRequest> for super::AutomationRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: AutomationRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::AutomationRequest> for AutomationRequest {
            fn from(value: super::AutomationRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct AutomationSnapshot {
            history:
                ::std::result::Result<::std::vec::Vec<super::HistoryEntry>, ::std::string::String>,
            occurrences: ::std::result::Result<
                ::std::vec::Vec<super::OccurrenceView>,
                ::std::string::String,
            >,
            rules: ::std::result::Result<::std::vec::Vec<super::Automation>, ::std::string::String>,
        }
        impl ::std::default::Default for AutomationSnapshot {
            fn default() -> Self {
                Self {
                    history: Err("no value supplied for history".to_string()),
                    occurrences: Err("no value supplied for occurrences".to_string()),
                    rules: Err("no value supplied for rules".to_string()),
                }
            }
        }
        impl AutomationSnapshot {
            pub fn history<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::HistoryEntry>>,
                T::Error: ::std::fmt::Display,
            {
                self.history = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for history: {e}"));
                self
            }
            pub fn occurrences<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::OccurrenceView>>,
                T::Error: ::std::fmt::Display,
            {
                self.occurrences = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for occurrences: {e}"));
                self
            }
            pub fn rules<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Automation>>,
                T::Error: ::std::fmt::Display,
            {
                self.rules = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for rules: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<AutomationSnapshot> for super::AutomationSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: AutomationSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    history: value.history?,
                    occurrences: value.occurrences?,
                    rules: value.rules?,
                })
            }
        }
        impl ::std::convert::From<super::AutomationSnapshot> for AutomationSnapshot {
            fn from(value: super::AutomationSnapshot) -> Self {
                Self {
                    history: Ok(value.history),
                    occurrences: Ok(value.occurrences),
                    rules: Ok(value.rules),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarEvent {
            all_day: ::std::result::Result<bool, ::std::string::String>,
            calendar: ::std::result::Result<::std::string::String, ::std::string::String>,
            color: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            ends_at: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            location: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            notes: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            revision: ::std::result::Result<i64, ::std::string::String>,
            source: ::std::result::Result<super::EventSource, ::std::string::String>,
            starts_at: ::std::result::Result<i64, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CalendarEvent {
            fn default() -> Self {
                Self {
                    all_day: Err("no value supplied for all_day".to_string()),
                    calendar: Err("no value supplied for calendar".to_string()),
                    color: Ok(Default::default()),
                    ends_at: Err("no value supplied for ends_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    location: Ok(Default::default()),
                    notes: Ok(Default::default()),
                    revision: Err("no value supplied for revision".to_string()),
                    source: Err("no value supplied for source".to_string()),
                    starts_at: Err("no value supplied for starts_at".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl CalendarEvent {
            pub fn all_day<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.all_day = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for all_day: {e}"));
                self
            }
            pub fn calendar<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.calendar = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for calendar: {e}"));
                self
            }
            pub fn color<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.color = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for color: {e}"));
                self
            }
            pub fn ends_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.ends_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ends_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn location<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.location = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for location: {e}"));
                self
            }
            pub fn notes<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.notes = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for notes: {e}"));
                self
            }
            pub fn revision<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.revision = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for revision: {e}"));
                self
            }
            pub fn source<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::EventSource>,
                T::Error: ::std::fmt::Display,
            {
                self.source = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for source: {e}"));
                self
            }
            pub fn starts_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.starts_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for starts_at: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CalendarEvent> for super::CalendarEvent {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarEvent,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    all_day: value.all_day?,
                    calendar: value.calendar?,
                    color: value.color?,
                    ends_at: value.ends_at?,
                    id: value.id?,
                    location: value.location?,
                    notes: value.notes?,
                    revision: value.revision?,
                    source: value.source?,
                    starts_at: value.starts_at?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarEvent> for CalendarEvent {
            fn from(value: super::CalendarEvent) -> Self {
                Self {
                    all_day: Ok(value.all_day),
                    calendar: Ok(value.calendar),
                    color: Ok(value.color),
                    ends_at: Ok(value.ends_at),
                    id: Ok(value.id),
                    location: Ok(value.location),
                    notes: Ok(value.notes),
                    revision: Ok(value.revision),
                    source: Ok(value.source),
                    starts_at: Ok(value.starts_at),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarImportRequest {
            events:
                ::std::result::Result<::std::vec::Vec<super::ImportedEvent>, ::std::string::String>,
            window_end: ::std::result::Result<i64, ::std::string::String>,
            window_start: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for CalendarImportRequest {
            fn default() -> Self {
                Self {
                    events: Err("no value supplied for events".to_string()),
                    window_end: Err("no value supplied for window_end".to_string()),
                    window_start: Err("no value supplied for window_start".to_string()),
                }
            }
        }
        impl CalendarImportRequest {
            pub fn events<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::ImportedEvent>>,
                T::Error: ::std::fmt::Display,
            {
                self.events = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for events: {e}"));
                self
            }
            pub fn window_end<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.window_end = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for window_end: {e}"));
                self
            }
            pub fn window_start<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.window_start = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for window_start: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CalendarImportRequest> for super::CalendarImportRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarImportRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    events: value.events?,
                    window_end: value.window_end?,
                    window_start: value.window_start?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarImportRequest> for CalendarImportRequest {
            fn from(value: super::CalendarImportRequest) -> Self {
                Self {
                    events: Ok(value.events),
                    window_end: Ok(value.window_end),
                    window_start: Ok(value.window_start),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarReceipt {
            result_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CalendarReceipt {
            fn default() -> Self {
                Self {
                    result_id: Err("no value supplied for result_id".to_string()),
                }
            }
        }
        impl CalendarReceipt {
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CalendarReceipt> for super::CalendarReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarReceipt> for CalendarReceipt {
            fn from(value: super::CalendarReceipt) -> Self {
                Self {
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarRequest {
            command: ::std::result::Result<super::CalendarCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CalendarRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl CalendarRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::CalendarCommand>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CalendarRequest> for super::CalendarRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarRequest> for CalendarRequest {
            fn from(value: super::CalendarRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarSnapshot {
            events:
                ::std::result::Result<::std::vec::Vec<super::CalendarEvent>, ::std::string::String>,
            last_import: ::std::result::Result<
                ::std::option::Option<super::ActionView>,
                ::std::string::String,
            >,
        }
        impl ::std::default::Default for CalendarSnapshot {
            fn default() -> Self {
                Self {
                    events: Err("no value supplied for events".to_string()),
                    last_import: Ok(Default::default()),
                }
            }
        }
        impl CalendarSnapshot {
            pub fn events<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::CalendarEvent>>,
                T::Error: ::std::fmt::Display,
            {
                self.events = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for events: {e}"));
                self
            }
            pub fn last_import<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<super::ActionView>>,
                T::Error: ::std::fmt::Display,
            {
                self.last_import = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for last_import: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CalendarSnapshot> for super::CalendarSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    events: value.events?,
                    last_import: value.last_import?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarSnapshot> for CalendarSnapshot {
            fn from(value: super::CalendarSnapshot) -> Self {
                Self {
                    events: Ok(value.events),
                    last_import: Ok(value.last_import),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CommandRequest {
            command: ::std::result::Result<super::Command, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CommandRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl CommandRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::Command>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CommandRequest> for super::CommandRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CommandRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::CommandRequest> for CommandRequest {
            fn from(value: super::CommandRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Comment {
            author_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            body: ::std::result::Result<::std::string::String, ::std::string::String>,
            created_at: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<i64, ::std::string::String>,
            ticket_id: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for Comment {
            fn default() -> Self {
                Self {
                    author_id: Err("no value supplied for author_id".to_string()),
                    body: Err("no value supplied for body".to_string()),
                    created_at: Err("no value supplied for created_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    ticket_id: Err("no value supplied for ticket_id".to_string()),
                }
            }
        }
        impl Comment {
            pub fn author_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.author_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for author_id: {e}"));
                self
            }
            pub fn body<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.body = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for body: {e}"));
                self
            }
            pub fn created_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.created_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for created_at: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn ticket_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.ticket_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ticket_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Comment> for super::Comment {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Comment,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    author_id: value.author_id?,
                    body: value.body?,
                    created_at: value.created_at?,
                    id: value.id?,
                    ticket_id: value.ticket_id?,
                })
            }
        }
        impl ::std::convert::From<super::Comment> for Comment {
            fn from(value: super::Comment) -> Self {
                Self {
                    author_id: Ok(value.author_id),
                    body: Ok(value.body),
                    created_at: Ok(value.created_at),
                    id: Ok(value.id),
                    ticket_id: Ok(value.ticket_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ConnectionStatus {
            account: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            auth_url: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            models: ::std::result::Result<::std::vec::Vec<super::Model>, ::std::string::String>,
            signing_in: ::std::result::Result<bool, ::std::string::String>,
        }
        impl ::std::default::Default for ConnectionStatus {
            fn default() -> Self {
                Self {
                    account: Ok(Default::default()),
                    auth_url: Ok(Default::default()),
                    error: Ok(Default::default()),
                    models: Err("no value supplied for models".to_string()),
                    signing_in: Err("no value supplied for signing_in".to_string()),
                }
            }
        }
        impl ConnectionStatus {
            pub fn account<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.account = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for account: {e}"));
                self
            }
            pub fn auth_url<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.auth_url = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for auth_url: {e}"));
                self
            }
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn models<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Model>>,
                T::Error: ::std::fmt::Display,
            {
                self.models = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for models: {e}"));
                self
            }
            pub fn signing_in<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.signing_in = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for signing_in: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ConnectionStatus> for super::ConnectionStatus {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ConnectionStatus,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    account: value.account?,
                    auth_url: value.auth_url?,
                    error: value.error?,
                    models: value.models?,
                    signing_in: value.signing_in?,
                })
            }
        }
        impl ::std::convert::From<super::ConnectionStatus> for ConnectionStatus {
            fn from(value: super::ConnectionStatus) -> Self {
                Self {
                    account: Ok(value.account),
                    auth_url: Ok(value.auth_url),
                    error: Ok(value.error),
                    models: Ok(value.models),
                    signing_in: Ok(value.signing_in),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Conversation {
            id: ::std::result::Result<i64, ::std::string::String>,
            snippet: ::std::result::Result<::std::string::String, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
            updated: ::std::result::Result<::std::string::String, ::std::string::String>,
            updated_at: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for Conversation {
            fn default() -> Self {
                Self {
                    id: Err("no value supplied for id".to_string()),
                    snippet: Err("no value supplied for snippet".to_string()),
                    title: Err("no value supplied for title".to_string()),
                    updated: Err("no value supplied for updated".to_string()),
                    updated_at: Err("no value supplied for updated_at".to_string()),
                }
            }
        }
        impl Conversation {
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn snippet<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.snippet = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for snippet: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
            pub fn updated<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.updated = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for updated: {e}"));
                self
            }
            pub fn updated_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.updated_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for updated_at: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Conversation> for super::Conversation {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Conversation,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    id: value.id?,
                    snippet: value.snippet?,
                    title: value.title?,
                    updated: value.updated?,
                    updated_at: value.updated_at?,
                })
            }
        }
        impl ::std::convert::From<super::Conversation> for Conversation {
            fn from(value: super::Conversation) -> Self {
                Self {
                    id: Ok(value.id),
                    snippet: Ok(value.snippet),
                    title: Ok(value.title),
                    updated: Ok(value.updated),
                    updated_at: Ok(value.updated_at),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CreateTerminalSession {
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CreateTerminalSession {
            fn default() -> Self {
                Self {
                    id: Err("no value supplied for id".to_string()),
                }
            }
        }
        impl CreateTerminalSession {
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<CreateTerminalSession> for super::CreateTerminalSession {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CreateTerminalSession,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self { id: value.id? })
            }
        }
        impl ::std::convert::From<super::CreateTerminalSession> for CreateTerminalSession {
            fn from(value: super::CreateTerminalSession) -> Self {
                Self { id: Ok(value.id) }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ErrorBody {
            code: ::std::result::Result<::std::string::String, ::std::string::String>,
            message: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for ErrorBody {
            fn default() -> Self {
                Self {
                    code: Err("no value supplied for code".to_string()),
                    message: Err("no value supplied for message".to_string()),
                }
            }
        }
        impl ErrorBody {
            pub fn code<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.code = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for code: {e}"));
                self
            }
            pub fn message<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.message = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for message: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ErrorBody> for super::ErrorBody {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ErrorBody,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    code: value.code?,
                    message: value.message?,
                })
            }
        }
        impl ::std::convert::From<super::ErrorBody> for ErrorBody {
            fn from(value: super::ErrorBody) -> Self {
                Self {
                    code: Ok(value.code),
                    message: Ok(value.message),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ExecutionPage {
            executions:
                ::std::result::Result<::std::vec::Vec<super::ExecutionView>, ::std::string::String>,
            next_page: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            ui_available: ::std::result::Result<bool, ::std::string::String>,
        }
        impl ::std::default::Default for ExecutionPage {
            fn default() -> Self {
                Self {
                    executions: Err("no value supplied for executions".to_string()),
                    next_page: Ok(Default::default()),
                    ui_available: Err("no value supplied for ui_available".to_string()),
                }
            }
        }
        impl ExecutionPage {
            pub fn executions<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::ExecutionView>>,
                T::Error: ::std::fmt::Display,
            {
                self.executions = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for executions: {e}"));
                self
            }
            pub fn next_page<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.next_page = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for next_page: {e}"));
                self
            }
            pub fn ui_available<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.ui_available = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ui_available: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ExecutionPage> for super::ExecutionPage {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ExecutionPage,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    executions: value.executions?,
                    next_page: value.next_page?,
                    ui_available: value.ui_available?,
                })
            }
        }
        impl ::std::convert::From<super::ExecutionPage> for ExecutionPage {
            fn from(value: super::ExecutionPage) -> Self {
                Self {
                    executions: Ok(value.executions),
                    next_page: Ok(value.next_page),
                    ui_available: Ok(value.ui_available),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ExecutionView {
            closed_at: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
            run_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            started_at: ::std::result::Result<i64, ::std::string::String>,
            status: ::std::result::Result<::std::string::String, ::std::string::String>,
            url: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            workflow_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            workflow_type: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for ExecutionView {
            fn default() -> Self {
                Self {
                    closed_at: Ok(Default::default()),
                    run_id: Err("no value supplied for run_id".to_string()),
                    started_at: Err("no value supplied for started_at".to_string()),
                    status: Err("no value supplied for status".to_string()),
                    url: Ok(Default::default()),
                    workflow_id: Err("no value supplied for workflow_id".to_string()),
                    workflow_type: Err("no value supplied for workflow_type".to_string()),
                }
            }
        }
        impl ExecutionView {
            pub fn closed_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.closed_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for closed_at: {e}"));
                self
            }
            pub fn run_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.run_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for run_id: {e}"));
                self
            }
            pub fn started_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.started_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for started_at: {e}"));
                self
            }
            pub fn status<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.status = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for status: {e}"));
                self
            }
            pub fn url<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.url = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for url: {e}"));
                self
            }
            pub fn workflow_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.workflow_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for workflow_id: {e}"));
                self
            }
            pub fn workflow_type<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.workflow_type = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for workflow_type: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ExecutionView> for super::ExecutionView {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ExecutionView,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    closed_at: value.closed_at?,
                    run_id: value.run_id?,
                    started_at: value.started_at?,
                    status: value.status?,
                    url: value.url?,
                    workflow_id: value.workflow_id?,
                    workflow_type: value.workflow_type?,
                })
            }
        }
        impl ::std::convert::From<super::ExecutionView> for ExecutionView {
            fn from(value: super::ExecutionView) -> Self {
                Self {
                    closed_at: Ok(value.closed_at),
                    run_id: Ok(value.run_id),
                    started_at: Ok(value.started_at),
                    status: Ok(value.status),
                    url: Ok(value.url),
                    workflow_id: Ok(value.workflow_id),
                    workflow_type: Ok(value.workflow_type),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Health {
            status: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Health {
            fn default() -> Self {
                Self {
                    status: Err("no value supplied for status".to_string()),
                }
            }
        }
        impl Health {
            pub fn status<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.status = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for status: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Health> for super::Health {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Health,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    status: value.status?,
                })
            }
        }
        impl ::std::convert::From<super::Health> for Health {
            fn from(value: super::Health) -> Self {
                Self {
                    status: Ok(value.status),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HistoryEntry {
            automation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            count: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<i64, ::std::string::String>,
            kind: ::std::result::Result<::std::string::String, ::std::string::String>,
            observed_at: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for HistoryEntry {
            fn default() -> Self {
                Self {
                    automation_id: Err("no value supplied for automation_id".to_string()),
                    count: Err("no value supplied for count".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    kind: Err("no value supplied for kind".to_string()),
                    observed_at: Err("no value supplied for observed_at".to_string()),
                }
            }
        }
        impl HistoryEntry {
            pub fn automation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.automation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for automation_id: {e}"));
                self
            }
            pub fn count<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.count = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for count: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn kind<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.kind = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for kind: {e}"));
                self
            }
            pub fn observed_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.observed_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for observed_at: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HistoryEntry> for super::HistoryEntry {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HistoryEntry,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    automation_id: value.automation_id?,
                    count: value.count?,
                    id: value.id?,
                    kind: value.kind?,
                    observed_at: value.observed_at?,
                })
            }
        }
        impl ::std::convert::From<super::HistoryEntry> for HistoryEntry {
            fn from(value: super::HistoryEntry) -> Self {
                Self {
                    automation_id: Ok(value.automation_id),
                    count: Ok(value.count),
                    id: Ok(value.id),
                    kind: Ok(value.kind),
                    observed_at: Ok(value.observed_at),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeClimate {
            action: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            ambient: ::std::result::Result<::std::option::Option<f64>, ::std::string::String>,
            gap: ::std::result::Result<i64, ::std::string::String>,
            max: ::std::result::Result<i64, ::std::string::String>,
            min: ::std::result::Result<i64, ::std::string::String>,
            mode: ::std::result::Result<super::ClimateMode, ::std::string::String>,
            pending: ::std::result::Result<bool, ::std::string::String>,
            target: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
            target_high: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
            target_low: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        }
        impl ::std::default::Default for HomeClimate {
            fn default() -> Self {
                Self {
                    action: Ok(Default::default()),
                    ambient: Ok(Default::default()),
                    gap: Err("no value supplied for gap".to_string()),
                    max: Err("no value supplied for max".to_string()),
                    min: Err("no value supplied for min".to_string()),
                    mode: Err("no value supplied for mode".to_string()),
                    pending: Err("no value supplied for pending".to_string()),
                    target: Ok(Default::default()),
                    target_high: Ok(Default::default()),
                    target_low: Ok(Default::default()),
                }
            }
        }
        impl HomeClimate {
            pub fn action<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.action = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for action: {e}"));
                self
            }
            pub fn ambient<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<f64>>,
                T::Error: ::std::fmt::Display,
            {
                self.ambient = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ambient: {e}"));
                self
            }
            pub fn gap<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.gap = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for gap: {e}"));
                self
            }
            pub fn max<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.max = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for max: {e}"));
                self
            }
            pub fn min<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.min = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for min: {e}"));
                self
            }
            pub fn mode<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::ClimateMode>,
                T::Error: ::std::fmt::Display,
            {
                self.mode = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for mode: {e}"));
                self
            }
            pub fn pending<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.pending = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for pending: {e}"));
                self
            }
            pub fn target<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.target = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for target: {e}"));
                self
            }
            pub fn target_high<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.target_high = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for target_high: {e}"));
                self
            }
            pub fn target_low<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.target_low = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for target_low: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeClimate> for super::HomeClimate {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeClimate,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    action: value.action?,
                    ambient: value.ambient?,
                    gap: value.gap?,
                    max: value.max?,
                    min: value.min?,
                    mode: value.mode?,
                    pending: value.pending?,
                    target: value.target?,
                    target_high: value.target_high?,
                    target_low: value.target_low?,
                })
            }
        }
        impl ::std::convert::From<super::HomeClimate> for HomeClimate {
            fn from(value: super::HomeClimate) -> Self {
                Self {
                    action: Ok(value.action),
                    ambient: Ok(value.ambient),
                    gap: Ok(value.gap),
                    max: Ok(value.max),
                    min: Ok(value.min),
                    mode: Ok(value.mode),
                    pending: Ok(value.pending),
                    target: Ok(value.target),
                    target_high: Ok(value.target_high),
                    target_low: Ok(value.target_low),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeConnection {
            access_token: ::std::result::Result<bool, ::std::string::String>,
            base_url: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for HomeConnection {
            fn default() -> Self {
                Self {
                    access_token: Err("no value supplied for access_token".to_string()),
                    base_url: Err("no value supplied for base_url".to_string()),
                }
            }
        }
        impl HomeConnection {
            pub fn access_token<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.access_token = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for access_token: {e}"));
                self
            }
            pub fn base_url<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.base_url = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for base_url: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeConnection> for super::HomeConnection {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeConnection,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    access_token: value.access_token?,
                    base_url: value.base_url?,
                })
            }
        }
        impl ::std::convert::From<super::HomeConnection> for HomeConnection {
            fn from(value: super::HomeConnection) -> Self {
                Self {
                    access_token: Ok(value.access_token),
                    base_url: Ok(value.base_url),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeConnectionRequest {
            access_client_id: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            access_client_secret: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            base_url: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for HomeConnectionRequest {
            fn default() -> Self {
                Self {
                    access_client_id: Ok(Default::default()),
                    access_client_secret: Ok(Default::default()),
                    base_url: Err("no value supplied for base_url".to_string()),
                }
            }
        }
        impl HomeConnectionRequest {
            pub fn access_client_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.access_client_id = value.try_into().map_err(|e| {
                    format!("error converting supplied value for access_client_id: {e}")
                });
                self
            }
            pub fn access_client_secret<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.access_client_secret = value.try_into().map_err(|e| {
                    format!("error converting supplied value for access_client_secret: {e}")
                });
                self
            }
            pub fn base_url<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.base_url = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for base_url: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeConnectionRequest> for super::HomeConnectionRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeConnectionRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    access_client_id: value.access_client_id?,
                    access_client_secret: value.access_client_secret?,
                    base_url: value.base_url?,
                })
            }
        }
        impl ::std::convert::From<super::HomeConnectionRequest> for HomeConnectionRequest {
            fn from(value: super::HomeConnectionRequest) -> Self {
                Self {
                    access_client_id: Ok(value.access_client_id),
                    access_client_secret: Ok(value.access_client_secret),
                    base_url: Ok(value.base_url),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeReceipt {
            result_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for HomeReceipt {
            fn default() -> Self {
                Self {
                    result_id: Err("no value supplied for result_id".to_string()),
                }
            }
        }
        impl HomeReceipt {
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeReceipt> for super::HomeReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::HomeReceipt> for HomeReceipt {
            fn from(value: super::HomeReceipt) -> Self {
                Self {
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeRequest {
            command: ::std::result::Result<super::HomeCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for HomeRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl HomeRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::HomeCommand>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeRequest> for super::HomeRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::HomeRequest> for HomeRequest {
            fn from(value: super::HomeRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeSnapshot {
            actions:
                ::std::result::Result<::std::vec::Vec<super::ActionView>, ::std::string::String>,
            climate: ::std::result::Result<
                ::std::option::Option<super::HomeClimate>,
                ::std::string::String,
            >,
            connection: ::std::result::Result<
                ::std::option::Option<super::HomeConnection>,
                ::std::string::String,
            >,
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            reachable: ::std::result::Result<bool, ::std::string::String>,
            switches:
                ::std::result::Result<::std::vec::Vec<super::HomeSwitch>, ::std::string::String>,
        }
        impl ::std::default::Default for HomeSnapshot {
            fn default() -> Self {
                Self {
                    actions: Err("no value supplied for actions".to_string()),
                    climate: Ok(Default::default()),
                    connection: Ok(Default::default()),
                    error: Ok(Default::default()),
                    reachable: Err("no value supplied for reachable".to_string()),
                    switches: Err("no value supplied for switches".to_string()),
                }
            }
        }
        impl HomeSnapshot {
            pub fn actions<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::ActionView>>,
                T::Error: ::std::fmt::Display,
            {
                self.actions = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for actions: {e}"));
                self
            }
            pub fn climate<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<super::HomeClimate>>,
                T::Error: ::std::fmt::Display,
            {
                self.climate = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for climate: {e}"));
                self
            }
            pub fn connection<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<super::HomeConnection>>,
                T::Error: ::std::fmt::Display,
            {
                self.connection = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for connection: {e}"));
                self
            }
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn reachable<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.reachable = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for reachable: {e}"));
                self
            }
            pub fn switches<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::HomeSwitch>>,
                T::Error: ::std::fmt::Display,
            {
                self.switches = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for switches: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeSnapshot> for super::HomeSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    actions: value.actions?,
                    climate: value.climate?,
                    connection: value.connection?,
                    error: value.error?,
                    reachable: value.reachable?,
                    switches: value.switches?,
                })
            }
        }
        impl ::std::convert::From<super::HomeSnapshot> for HomeSnapshot {
            fn from(value: super::HomeSnapshot) -> Self {
                Self {
                    actions: Ok(value.actions),
                    climate: Ok(value.climate),
                    connection: Ok(value.connection),
                    error: Ok(value.error),
                    reachable: Ok(value.reachable),
                    switches: Ok(value.switches),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct HomeSwitch {
            key: ::std::result::Result<super::SwitchKey, ::std::string::String>,
            label: ::std::result::Result<::std::string::String, ::std::string::String>,
            lit: ::std::result::Result<i64, ::std::string::String>,
            members:
                ::std::result::Result<::std::vec::Vec<super::SwitchKey>, ::std::string::String>,
            on: ::std::result::Result<bool, ::std::string::String>,
            pending: ::std::result::Result<bool, ::std::string::String>,
            room: ::std::result::Result<::std::string::String, ::std::string::String>,
            total: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for HomeSwitch {
            fn default() -> Self {
                Self {
                    key: Err("no value supplied for key".to_string()),
                    label: Err("no value supplied for label".to_string()),
                    lit: Err("no value supplied for lit".to_string()),
                    members: Err("no value supplied for members".to_string()),
                    on: Err("no value supplied for on".to_string()),
                    pending: Err("no value supplied for pending".to_string()),
                    room: Err("no value supplied for room".to_string()),
                    total: Err("no value supplied for total".to_string()),
                }
            }
        }
        impl HomeSwitch {
            pub fn key<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::SwitchKey>,
                T::Error: ::std::fmt::Display,
            {
                self.key = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for key: {e}"));
                self
            }
            pub fn label<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.label = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for label: {e}"));
                self
            }
            pub fn lit<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.lit = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for lit: {e}"));
                self
            }
            pub fn members<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::SwitchKey>>,
                T::Error: ::std::fmt::Display,
            {
                self.members = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for members: {e}"));
                self
            }
            pub fn on<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.on = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for on: {e}"));
                self
            }
            pub fn pending<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.pending = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for pending: {e}"));
                self
            }
            pub fn room<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.room = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for room: {e}"));
                self
            }
            pub fn total<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.total = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for total: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<HomeSwitch> for super::HomeSwitch {
            type Error = super::error::ConversionError;
            fn try_from(
                value: HomeSwitch,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    key: value.key?,
                    label: value.label?,
                    lit: value.lit?,
                    members: value.members?,
                    on: value.on?,
                    pending: value.pending?,
                    room: value.room?,
                    total: value.total?,
                })
            }
        }
        impl ::std::convert::From<super::HomeSwitch> for HomeSwitch {
            fn from(value: super::HomeSwitch) -> Self {
                Self {
                    key: Ok(value.key),
                    label: Ok(value.label),
                    lit: Ok(value.lit),
                    members: Ok(value.members),
                    on: Ok(value.on),
                    pending: Ok(value.pending),
                    room: Ok(value.room),
                    total: Ok(value.total),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ImportedEvent {
            all_day: ::std::result::Result<bool, ::std::string::String>,
            calendar: ::std::result::Result<::std::string::String, ::std::string::String>,
            color: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            ends_at: ::std::result::Result<i64, ::std::string::String>,
            external_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            location: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            notes: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            starts_at: ::std::result::Result<i64, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for ImportedEvent {
            fn default() -> Self {
                Self {
                    all_day: Err("no value supplied for all_day".to_string()),
                    calendar: Err("no value supplied for calendar".to_string()),
                    color: Ok(Default::default()),
                    ends_at: Err("no value supplied for ends_at".to_string()),
                    external_id: Err("no value supplied for external_id".to_string()),
                    location: Ok(Default::default()),
                    notes: Ok(Default::default()),
                    starts_at: Err("no value supplied for starts_at".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl ImportedEvent {
            pub fn all_day<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.all_day = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for all_day: {e}"));
                self
            }
            pub fn calendar<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.calendar = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for calendar: {e}"));
                self
            }
            pub fn color<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.color = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for color: {e}"));
                self
            }
            pub fn ends_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.ends_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ends_at: {e}"));
                self
            }
            pub fn external_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.external_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for external_id: {e}"));
                self
            }
            pub fn location<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.location = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for location: {e}"));
                self
            }
            pub fn notes<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.notes = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for notes: {e}"));
                self
            }
            pub fn starts_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.starts_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for starts_at: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<ImportedEvent> for super::ImportedEvent {
            type Error = super::error::ConversionError;
            fn try_from(
                value: ImportedEvent,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    all_day: value.all_day?,
                    calendar: value.calendar?,
                    color: value.color?,
                    ends_at: value.ends_at?,
                    external_id: value.external_id?,
                    location: value.location?,
                    notes: value.notes?,
                    starts_at: value.starts_at?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::ImportedEvent> for ImportedEvent {
            fn from(value: super::ImportedEvent) -> Self {
                Self {
                    all_day: Ok(value.all_day),
                    calendar: Ok(value.calendar),
                    color: Ok(value.color),
                    ends_at: Ok(value.ends_at),
                    external_id: Ok(value.external_id),
                    location: Ok(value.location),
                    notes: Ok(value.notes),
                    starts_at: Ok(value.starts_at),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Model {
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            name: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Model {
            fn default() -> Self {
                Self {
                    id: Err("no value supplied for id".to_string()),
                    name: Err("no value supplied for name".to_string()),
                }
            }
        }
        impl Model {
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn name<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.name = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for name: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Model> for super::Model {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Model,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    id: value.id?,
                    name: value.name?,
                })
            }
        }
        impl ::std::convert::From<super::Model> for Model {
            fn from(value: super::Model) -> Self {
                Self {
                    id: Ok(value.id),
                    name: Ok(value.name),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct OccurrenceView {
            automation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            detail: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            scheduled_at: ::std::result::Result<i64, ::std::string::String>,
            state: ::std::result::Result<::std::string::String, ::std::string::String>,
            ticket_id: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        }
        impl ::std::default::Default for OccurrenceView {
            fn default() -> Self {
                Self {
                    automation_id: Err("no value supplied for automation_id".to_string()),
                    detail: Ok(Default::default()),
                    id: Err("no value supplied for id".to_string()),
                    scheduled_at: Err("no value supplied for scheduled_at".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    ticket_id: Ok(Default::default()),
                }
            }
        }
        impl OccurrenceView {
            pub fn automation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.automation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for automation_id: {e}"));
                self
            }
            pub fn detail<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.detail = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for detail: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn scheduled_at<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.scheduled_at = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for scheduled_at: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn ticket_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.ticket_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ticket_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<OccurrenceView> for super::OccurrenceView {
            type Error = super::error::ConversionError;
            fn try_from(
                value: OccurrenceView,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    automation_id: value.automation_id?,
                    detail: value.detail?,
                    id: value.id?,
                    scheduled_at: value.scheduled_at?,
                    state: value.state?,
                    ticket_id: value.ticket_id?,
                })
            }
        }
        impl ::std::convert::From<super::OccurrenceView> for OccurrenceView {
            fn from(value: super::OccurrenceView) -> Self {
                Self {
                    automation_id: Ok(value.automation_id),
                    detail: Ok(value.detail),
                    id: Ok(value.id),
                    scheduled_at: Ok(value.scheduled_at),
                    state: Ok(value.state),
                    ticket_id: Ok(value.ticket_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Settings {
            model: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            selected_conversation:
                ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        }
        impl ::std::default::Default for Settings {
            fn default() -> Self {
                Self {
                    model: Ok(Default::default()),
                    selected_conversation: Ok(Default::default()),
                }
            }
        }
        impl Settings {
            pub fn model<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.model = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for model: {e}"));
                self
            }
            pub fn selected_conversation<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.selected_conversation = value.try_into().map_err(|e| {
                    format!("error converting supplied value for selected_conversation: {e}")
                });
                self
            }
        }
        impl ::std::convert::TryFrom<Settings> for super::Settings {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Settings,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    model: value.model?,
                    selected_conversation: value.selected_conversation?,
                })
            }
        }
        impl ::std::convert::From<super::Settings> for Settings {
            fn from(value: super::Settings) -> Self {
                Self {
                    model: Ok(value.model),
                    selected_conversation: Ok(value.selected_conversation),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Snapshot {
            conversations:
                ::std::result::Result<::std::vec::Vec<super::Conversation>, ::std::string::String>,
            settings: ::std::result::Result<super::Settings, ::std::string::String>,
            todos: ::std::result::Result<::std::vec::Vec<super::Todo>, ::std::string::String>,
            turns: ::std::result::Result<::std::vec::Vec<super::Turn>, ::std::string::String>,
        }
        impl ::std::default::Default for Snapshot {
            fn default() -> Self {
                Self {
                    conversations: Err("no value supplied for conversations".to_string()),
                    settings: Err("no value supplied for settings".to_string()),
                    todos: Err("no value supplied for todos".to_string()),
                    turns: Err("no value supplied for turns".to_string()),
                }
            }
        }
        impl Snapshot {
            pub fn conversations<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Conversation>>,
                T::Error: ::std::fmt::Display,
            {
                self.conversations = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for conversations: {e}"));
                self
            }
            pub fn settings<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::Settings>,
                T::Error: ::std::fmt::Display,
            {
                self.settings = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for settings: {e}"));
                self
            }
            pub fn todos<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Todo>>,
                T::Error: ::std::fmt::Display,
            {
                self.todos = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for todos: {e}"));
                self
            }
            pub fn turns<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Turn>>,
                T::Error: ::std::fmt::Display,
            {
                self.turns = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for turns: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Snapshot> for super::Snapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Snapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    conversations: value.conversations?,
                    settings: value.settings?,
                    todos: value.todos?,
                    turns: value.turns?,
                })
            }
        }
        impl ::std::convert::From<super::Snapshot> for Snapshot {
            fn from(value: super::Snapshot) -> Self {
                Self {
                    conversations: Ok(value.conversations),
                    settings: Ok(value.settings),
                    todos: Ok(value.todos),
                    turns: Ok(value.turns),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TerminalSession {
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            state: ::std::result::Result<::std::string::String, ::std::string::String>,
            workspace_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for TerminalSession {
            fn default() -> Self {
                Self {
                    id: Err("no value supplied for id".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    workspace_id: Err("no value supplied for workspace_id".to_string()),
                }
            }
        }
        impl TerminalSession {
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn workspace_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.workspace_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for workspace_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TerminalSession> for super::TerminalSession {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TerminalSession,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    id: value.id?,
                    state: value.state?,
                    workspace_id: value.workspace_id?,
                })
            }
        }
        impl ::std::convert::From<super::TerminalSession> for TerminalSession {
            fn from(value: super::TerminalSession) -> Self {
                Self {
                    id: Ok(value.id),
                    state: Ok(value.state),
                    workspace_id: Ok(value.workspace_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Ticket {
            assignee_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            assignee_kind: ::std::result::Result<super::AssigneeKind, ::std::string::String>,
            generation: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<i64, ::std::string::String>,
            revision: ::std::result::Result<i64, ::std::string::String>,
            status: ::std::result::Result<super::TicketStatus, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Ticket {
            fn default() -> Self {
                Self {
                    assignee_id: Err("no value supplied for assignee_id".to_string()),
                    assignee_kind: Err("no value supplied for assignee_kind".to_string()),
                    generation: Err("no value supplied for generation".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    revision: Err("no value supplied for revision".to_string()),
                    status: Err("no value supplied for status".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl Ticket {
            pub fn assignee_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.assignee_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for assignee_id: {e}"));
                self
            }
            pub fn assignee_kind<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::AssigneeKind>,
                T::Error: ::std::fmt::Display,
            {
                self.assignee_kind = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for assignee_kind: {e}"));
                self
            }
            pub fn generation<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.generation = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for generation: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn revision<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.revision = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for revision: {e}"));
                self
            }
            pub fn status<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TicketStatus>,
                T::Error: ::std::fmt::Display,
            {
                self.status = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for status: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Ticket> for super::Ticket {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Ticket,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    assignee_id: value.assignee_id?,
                    assignee_kind: value.assignee_kind?,
                    generation: value.generation?,
                    id: value.id?,
                    revision: value.revision?,
                    status: value.status?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::Ticket> for Ticket {
            fn from(value: super::Ticket) -> Self {
                Self {
                    assignee_id: Ok(value.assignee_id),
                    assignee_kind: Ok(value.assignee_kind),
                    generation: Ok(value.generation),
                    id: Ok(value.id),
                    revision: Ok(value.revision),
                    status: Ok(value.status),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TicketCommandRequest {
            command: ::std::result::Result<super::TicketCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for TicketCommandRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl TicketCommandRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TicketCommand>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TicketCommandRequest> for super::TicketCommandRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TicketCommandRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::TicketCommandRequest> for TicketCommandRequest {
            fn from(value: super::TicketCommandRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TicketContract {
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for TicketContract {
            fn default() -> Self {
                Self {
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl TicketContract {
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TicketContract> for super::TicketContract {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TicketContract,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::TicketContract> for TicketContract {
            fn from(value: super::TicketContract) -> Self {
                Self {
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TicketProposal {
            agent_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for TicketProposal {
            fn default() -> Self {
                Self {
                    agent_id: Err("no value supplied for agent_id".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl TicketProposal {
            pub fn agent_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.agent_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for agent_id: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TicketProposal> for super::TicketProposal {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TicketProposal,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    agent_id: value.agent_id?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::TicketProposal> for TicketProposal {
            fn from(value: super::TicketProposal) -> Self {
                Self {
                    agent_id: Ok(value.agent_id),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TicketReceipt {
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            result_id: ::std::result::Result<::std::option::Option<i64>, ::std::string::String>,
        }
        impl ::std::default::Default for TicketReceipt {
            fn default() -> Self {
                Self {
                    operation_id: Err("no value supplied for operation_id".to_string()),
                    result_id: Ok(Default::default()),
                }
            }
        }
        impl TicketReceipt {
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<i64>>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TicketReceipt> for super::TicketReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TicketReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    operation_id: value.operation_id?,
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::TicketReceipt> for TicketReceipt {
            fn from(value: super::TicketReceipt) -> Self {
                Self {
                    operation_id: Ok(value.operation_id),
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TicketSnapshot {
            assignees:
                ::std::result::Result<::std::vec::Vec<super::Assignee>, ::std::string::String>,
            comments: ::std::result::Result<::std::vec::Vec<super::Comment>, ::std::string::String>,
            runs: ::std::result::Result<::std::vec::Vec<super::WorkRun>, ::std::string::String>,
            tickets: ::std::result::Result<::std::vec::Vec<super::Ticket>, ::std::string::String>,
        }
        impl ::std::default::Default for TicketSnapshot {
            fn default() -> Self {
                Self {
                    assignees: Err("no value supplied for assignees".to_string()),
                    comments: Err("no value supplied for comments".to_string()),
                    runs: Err("no value supplied for runs".to_string()),
                    tickets: Err("no value supplied for tickets".to_string()),
                }
            }
        }
        impl TicketSnapshot {
            pub fn assignees<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Assignee>>,
                T::Error: ::std::fmt::Display,
            {
                self.assignees = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for assignees: {e}"));
                self
            }
            pub fn comments<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Comment>>,
                T::Error: ::std::fmt::Display,
            {
                self.comments = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for comments: {e}"));
                self
            }
            pub fn runs<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::WorkRun>>,
                T::Error: ::std::fmt::Display,
            {
                self.runs = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for runs: {e}"));
                self
            }
            pub fn tickets<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Ticket>>,
                T::Error: ::std::fmt::Display,
            {
                self.tickets = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for tickets: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TicketSnapshot> for super::TicketSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TicketSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    assignees: value.assignees?,
                    comments: value.comments?,
                    runs: value.runs?,
                    tickets: value.tickets?,
                })
            }
        }
        impl ::std::convert::From<super::TicketSnapshot> for TicketSnapshot {
            fn from(value: super::TicketSnapshot) -> Self {
                Self {
                    assignees: Ok(value.assignees),
                    comments: Ok(value.comments),
                    runs: Ok(value.runs),
                    tickets: Ok(value.tickets),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Todo {
            completed: ::std::result::Result<bool, ::std::string::String>,
            id: ::std::result::Result<i64, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Todo {
            fn default() -> Self {
                Self {
                    completed: Err("no value supplied for completed".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl Todo {
            pub fn completed<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<bool>,
                T::Error: ::std::fmt::Display,
            {
                self.completed = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for completed: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn title<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.title = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for title: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Todo> for super::Todo {
            type Error = super::error::ConversionError;
            fn try_from(value: Todo) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    completed: value.completed?,
                    id: value.id?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::Todo> for Todo {
            fn from(value: super::Todo) -> Self {
                Self {
                    completed: Ok(value.completed),
                    id: Ok(value.id),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Turn {
            conversation_id: ::std::result::Result<i64, ::std::string::String>,
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            id: ::std::result::Result<i64, ::std::string::String>,
            prompt: ::std::result::Result<::std::string::String, ::std::string::String>,
            response: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            state: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Turn {
            fn default() -> Self {
                Self {
                    conversation_id: Err("no value supplied for conversation_id".to_string()),
                    error: Ok(Default::default()),
                    id: Err("no value supplied for id".to_string()),
                    prompt: Err("no value supplied for prompt".to_string()),
                    response: Ok(Default::default()),
                    state: Err("no value supplied for state".to_string()),
                }
            }
        }
        impl Turn {
            pub fn conversation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.conversation_id = value.try_into().map_err(|e| {
                    format!("error converting supplied value for conversation_id: {e}")
                });
                self
            }
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn prompt<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.prompt = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for prompt: {e}"));
                self
            }
            pub fn response<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.response = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for response: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Turn> for super::Turn {
            type Error = super::error::ConversionError;
            fn try_from(value: Turn) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    conversation_id: value.conversation_id?,
                    error: value.error?,
                    id: value.id?,
                    prompt: value.prompt?,
                    response: value.response?,
                    state: value.state?,
                })
            }
        }
        impl ::std::convert::From<super::Turn> for Turn {
            fn from(value: super::Turn) -> Self {
                Self {
                    conversation_id: Ok(value.conversation_id),
                    error: Ok(value.error),
                    id: Ok(value.id),
                    prompt: Ok(value.prompt),
                    response: Ok(value.response),
                    state: Ok(value.state),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Version {
            api: ::std::result::Result<i32, ::std::string::String>,
            product: ::std::result::Result<::std::string::String, ::std::string::String>,
            version: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Version {
            fn default() -> Self {
                Self {
                    api: Err("no value supplied for api".to_string()),
                    product: Err("no value supplied for product".to_string()),
                    version: Err("no value supplied for version".to_string()),
                }
            }
        }
        impl Version {
            pub fn api<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i32>,
                T::Error: ::std::fmt::Display,
            {
                self.api = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for api: {e}"));
                self
            }
            pub fn product<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.product = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for product: {e}"));
                self
            }
            pub fn version<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.version = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for version: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Version> for super::Version {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Version,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    api: value.api?,
                    product: value.product?,
                    version: value.version?,
                })
            }
        }
        impl ::std::convert::From<super::Version> for Version {
            fn from(value: super::Version) -> Self {
                Self {
                    api: Ok(value.api),
                    product: Ok(value.product),
                    version: Ok(value.version),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct WorkRun {
            error: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            generation: ::std::result::Result<i64, ::std::string::String>,
            run_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            state: ::std::result::Result<::std::string::String, ::std::string::String>,
            ticket_id: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for WorkRun {
            fn default() -> Self {
                Self {
                    error: Ok(Default::default()),
                    generation: Err("no value supplied for generation".to_string()),
                    run_id: Err("no value supplied for run_id".to_string()),
                    state: Err("no value supplied for state".to_string()),
                    ticket_id: Err("no value supplied for ticket_id".to_string()),
                }
            }
        }
        impl WorkRun {
            pub fn error<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.error = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for error: {e}"));
                self
            }
            pub fn generation<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.generation = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for generation: {e}"));
                self
            }
            pub fn run_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.run_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for run_id: {e}"));
                self
            }
            pub fn state<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.state = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for state: {e}"));
                self
            }
            pub fn ticket_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.ticket_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for ticket_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<WorkRun> for super::WorkRun {
            type Error = super::error::ConversionError;
            fn try_from(
                value: WorkRun,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    error: value.error?,
                    generation: value.generation?,
                    run_id: value.run_id?,
                    state: value.state?,
                    ticket_id: value.ticket_id?,
                })
            }
        }
        impl ::std::convert::From<super::WorkRun> for WorkRun {
            fn from(value: super::WorkRun) -> Self {
                Self {
                    error: Ok(value.error),
                    generation: Ok(value.generation),
                    run_id: Ok(value.run_id),
                    state: Ok(value.state),
                    ticket_id: Ok(value.ticket_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct Workspace {
            color: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            icon: ::std::result::Result<
                ::std::option::Option<::std::string::String>,
                ::std::string::String,
            >,
            id: ::std::result::Result<::std::string::String, ::std::string::String>,
            name: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for Workspace {
            fn default() -> Self {
                Self {
                    color: Ok(Default::default()),
                    icon: Ok(Default::default()),
                    id: Err("no value supplied for id".to_string()),
                    name: Err("no value supplied for name".to_string()),
                }
            }
        }
        impl Workspace {
            pub fn color<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.color = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for color: {e}"));
                self
            }
            pub fn icon<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::option::Option<::std::string::String>>,
                T::Error: ::std::fmt::Display,
            {
                self.icon = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for icon: {e}"));
                self
            }
            pub fn id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
                self
            }
            pub fn name<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.name = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for name: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<Workspace> for super::Workspace {
            type Error = super::error::ConversionError;
            fn try_from(
                value: Workspace,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    color: value.color?,
                    icon: value.icon?,
                    id: value.id?,
                    name: value.name?,
                })
            }
        }
        impl ::std::convert::From<super::Workspace> for Workspace {
            fn from(value: super::Workspace) -> Self {
                Self {
                    color: Ok(value.color),
                    icon: Ok(value.icon),
                    id: Ok(value.id),
                    name: Ok(value.name),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct WorkspaceReceipt {
            result_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for WorkspaceReceipt {
            fn default() -> Self {
                Self {
                    result_id: Err("no value supplied for result_id".to_string()),
                }
            }
        }
        impl WorkspaceReceipt {
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<WorkspaceReceipt> for super::WorkspaceReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: WorkspaceReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::WorkspaceReceipt> for WorkspaceReceipt {
            fn from(value: super::WorkspaceReceipt) -> Self {
                Self {
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct WorkspaceRequest {
            command: ::std::result::Result<super::WorkspaceCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for WorkspaceRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl WorkspaceRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::WorkspaceCommand>,
                T::Error: ::std::fmt::Display,
            {
                self.command = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for command: {e}"));
                self
            }
            pub fn operation_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.operation_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for operation_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<WorkspaceRequest> for super::WorkspaceRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: WorkspaceRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::WorkspaceRequest> for WorkspaceRequest {
            fn from(value: super::WorkspaceRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct WorkspaceState {
            current_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            workspaces:
                ::std::result::Result<::std::vec::Vec<super::Workspace>, ::std::string::String>,
        }
        impl ::std::default::Default for WorkspaceState {
            fn default() -> Self {
                Self {
                    current_id: Err("no value supplied for current_id".to_string()),
                    workspaces: Err("no value supplied for workspaces".to_string()),
                }
            }
        }
        impl WorkspaceState {
            pub fn current_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::string::String>,
                T::Error: ::std::fmt::Display,
            {
                self.current_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for current_id: {e}"));
                self
            }
            pub fn workspaces<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<::std::vec::Vec<super::Workspace>>,
                T::Error: ::std::fmt::Display,
            {
                self.workspaces = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for workspaces: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<WorkspaceState> for super::WorkspaceState {
            type Error = super::error::ConversionError;
            fn try_from(
                value: WorkspaceState,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    current_id: value.current_id?,
                    workspaces: value.workspaces?,
                })
            }
        }
        impl ::std::convert::From<super::WorkspaceState> for WorkspaceState {
            fn from(value: super::WorkspaceState) -> Self {
                Self {
                    current_id: Ok(value.current_id),
                    workspaces: Ok(value.workspaces),
                }
            }
        }
    }
}
#[derive(Clone, Debug)]
/*Client for ainc-daemon



Version: 0.3.4*/
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}
impl Client {
    /// Create a new client.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new(baseurl: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let client = {
            let dur = ::std::time::Duration::from_secs(15u64);
            reqwest::ClientBuilder::new()
                .connect_timeout(dur)
                .timeout(dur)
        };
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::ClientBuilder::new();
        Self::new_with_client(baseurl, client.build().unwrap())
    }
    /// Construct a new client with an existing `reqwest::Client`,
    /// allowing more control over its configuration.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}
impl ClientInfo<()> for Client {
    fn api_version() -> &'static str {
        "0.3.4"
    }
    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    fn inner(&self) -> &() {
        &()
    }
}
impl ClientHooks<()> for &Client {}
impl Client {
    /*Sends a `GET` request to `/health/live`

    ```ignore
    let response = client.health_live()
        .send()
        .await;
    ```*/
    pub fn health_live(&self) -> builder::HealthLive<'_> {
        builder::HealthLive::new(self)
    }
    /*Sends a `GET` request to `/health/ready`

    ```ignore
    let response = client.health_ready()
        .send()
        .await;
    ```*/
    pub fn health_ready(&self) -> builder::HealthReady<'_> {
        builder::HealthReady::new(self)
    }
    /*Sends a `GET` request to `/v1/automations`

    ```ignore
    let response = client.automations_state()
        .send()
        .await;
    ```*/
    pub fn automations_state(&self) -> builder::AutomationsState<'_> {
        builder::AutomationsState::new(self)
    }
    /*Sends a `POST` request to `/v1/automations/commands`

    ```ignore
    let response = client.automations_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn automations_command(&self) -> builder::AutomationsCommand<'_> {
        builder::AutomationsCommand::new(self)
    }
    /*Sends a `GET` request to `/v1/calendar`

    Arguments:
    - `from`: Unix seconds; defaults to 31 days ago.
    - `to`: Unix seconds, exclusive; defaults to 180 days ahead.
    ```ignore
    let response = client.calendar_state()
        .from(from)
        .to(to)
        .send()
        .await;
    ```*/
    pub fn calendar_state(&self) -> builder::CalendarState<'_> {
        builder::CalendarState::new(self)
    }
    /*Sends a `POST` request to `/v1/calendar/commands`

    ```ignore
    let response = client.calendar_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn calendar_command(&self) -> builder::CalendarCommand<'_> {
        builder::CalendarCommand::new(self)
    }
    /*Sends a `POST` request to `/v1/calendar/imports`

    ```ignore
    let response = client.calendar_import()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn calendar_import(&self) -> builder::CalendarImport<'_> {
        builder::CalendarImport::new(self)
    }
    /*Sends a `POST` request to `/v1/commands`

    ```ignore
    let response = client.product_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn product_command(&self) -> builder::ProductCommand<'_> {
        builder::ProductCommand::new(self)
    }
    /*Sends a `GET` request to `/v1/connection`

    ```ignore
    let response = client.connection_status()
        .send()
        .await;
    ```*/
    pub fn connection_status(&self) -> builder::ConnectionStatus<'_> {
        builder::ConnectionStatus::new(self)
    }
    /*Sends a `POST` request to `/v1/connection/cancel`

    ```ignore
    let response = client.connection_cancel()
        .send()
        .await;
    ```*/
    pub fn connection_cancel(&self) -> builder::ConnectionCancel<'_> {
        builder::ConnectionCancel::new(self)
    }
    /*Sends a `POST` request to `/v1/connection/login`

    ```ignore
    let response = client.connection_login()
        .send()
        .await;
    ```*/
    pub fn connection_login(&self) -> builder::ConnectionLogin<'_> {
        builder::ConnectionLogin::new(self)
    }
    /*Sends a `POST` request to `/v1/connection/logout`

    ```ignore
    let response = client.connection_logout()
        .send()
        .await;
    ```*/
    pub fn connection_logout(&self) -> builder::ConnectionLogout<'_> {
        builder::ConnectionLogout::new(self)
    }
    /*Sends a `GET` request to `/v1/home`

    ```ignore
    let response = client.home_state()
        .send()
        .await;
    ```*/
    pub fn home_state(&self) -> builder::HomeState<'_> {
        builder::HomeState::new(self)
    }
    /*Sends a `POST` request to `/v1/home/commands`

    ```ignore
    let response = client.home_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn home_command(&self) -> builder::HomeCommand<'_> {
        builder::HomeCommand::new(self)
    }
    /*Sends a `PUT` request to `/v1/home/connection`

    ```ignore
    let response = client.home_connect()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn home_connect(&self) -> builder::HomeConnect<'_> {
        builder::HomeConnect::new(self)
    }
    /*Sends a `DELETE` request to `/v1/home/connection`

    ```ignore
    let response = client.home_disconnect()
        .send()
        .await;
    ```*/
    pub fn home_disconnect(&self) -> builder::HomeDisconnect<'_> {
        builder::HomeDisconnect::new(self)
    }
    /*Sends a `GET` request to `/v1/state`

    ```ignore
    let response = client.product_state()
        .send()
        .await;
    ```*/
    pub fn product_state(&self) -> builder::ProductState<'_> {
        builder::ProductState::new(self)
    }
    /*Sends a `GET` request to `/v1/temporal/executions`

    Arguments:
    - `page`: Opaque next-page token
    - `status`: All, Running, Completed, Failed, Canceled, Terminated, TimedOut, ContinuedAsNew, or Paused
    ```ignore
    let response = client.temporal_executions()
        .page(page)
        .status(status)
        .send()
        .await;
    ```*/
    pub fn temporal_executions(&self) -> builder::TemporalExecutions<'_> {
        builder::TemporalExecutions::new(self)
    }
    /*Sends a `GET` request to `/v1/terminal/sessions`

    ```ignore
    let response = client.terminal_sessions_list()
        .send()
        .await;
    ```*/
    pub fn terminal_sessions_list(&self) -> builder::TerminalSessionsList<'_> {
        builder::TerminalSessionsList::new(self)
    }
    /*Sends a `POST` request to `/v1/terminal/sessions`

    ```ignore
    let response = client.terminal_sessions_create()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn terminal_sessions_create(&self) -> builder::TerminalSessionsCreate<'_> {
        builder::TerminalSessionsCreate::new(self)
    }
    /*Sends a `DELETE` request to `/v1/terminal/sessions/{id}`

    ```ignore
    let response = client.terminal_sessions_close()
        .id(id)
        .send()
        .await;
    ```*/
    pub fn terminal_sessions_close(&self) -> builder::TerminalSessionsClose<'_> {
        builder::TerminalSessionsClose::new(self)
    }
    /*Sends a `GET` request to `/v1/tickets`

    ```ignore
    let response = client.tickets_state()
        .send()
        .await;
    ```*/
    pub fn tickets_state(&self) -> builder::TicketsState<'_> {
        builder::TicketsState::new(self)
    }
    /*Sends a `POST` request to `/v1/tickets/commands`

    ```ignore
    let response = client.tickets_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn tickets_command(&self) -> builder::TicketsCommand<'_> {
        builder::TicketsCommand::new(self)
    }
    /*Sends a `POST` request to `/v1/tickets/contract`

    ```ignore
    let response = client.ticket_contract()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn ticket_contract(&self) -> builder::TicketContract<'_> {
        builder::TicketContract::new(self)
    }
    /*Sends a `GET` request to `/v1/workspaces`

    ```ignore
    let response = client.workspaces_state()
        .send()
        .await;
    ```*/
    pub fn workspaces_state(&self) -> builder::WorkspacesState<'_> {
        builder::WorkspacesState::new(self)
    }
    /*Sends a `POST` request to `/v1/workspaces/commands`

    ```ignore
    let response = client.workspaces_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn workspaces_command(&self) -> builder::WorkspacesCommand<'_> {
        builder::WorkspacesCommand::new(self)
    }
    /*Sends a `GET` request to `/version`

    ```ignore
    let response = client.get_version()
        .send()
        .await;
    ```*/
    pub fn get_version(&self) -> builder::GetVersion<'_> {
        builder::GetVersion::new(self)
    }
}
/// Types for composing operation parameters.
#[allow(clippy::all)]
pub mod builder {
    use super::types;
    #[allow(unused_imports)]
    use super::{
        ByteStream, ClientHooks, ClientInfo, Error, OperationInfo, RequestBuilderExt,
        ResponseValue, encode_path,
    };
    /*Builder for [`Client::health_live`]

    [`Client::health_live`]: super::Client::health_live*/
    #[derive(Debug, Clone)]
    pub struct HealthLive<'a> {
        client: &'a super::Client,
    }
    impl<'a> HealthLive<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/health/live`
        pub async fn send(self) -> Result<ResponseValue<types::Health>, Error<()>> {
            let Self { client } = self;
            let url = format!("{}/health/live", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "health_live",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::health_ready`]

    [`Client::health_ready`]: super::Client::health_ready*/
    #[derive(Debug, Clone)]
    pub struct HealthReady<'a> {
        client: &'a super::Client,
    }
    impl<'a> HealthReady<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/health/ready`
        pub async fn send(self) -> Result<ResponseValue<types::Health>, Error<()>> {
            let Self { client } = self;
            let url = format!("{}/health/ready", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "health_ready",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                503u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::automations_state`]

    [`Client::automations_state`]: super::Client::automations_state*/
    #[derive(Debug, Clone)]
    pub struct AutomationsState<'a> {
        client: &'a super::Client,
    }
    impl<'a> AutomationsState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/automations`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::AutomationSnapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/automations", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "automations_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::automations_command`]

    [`Client::automations_command`]: super::Client::automations_command*/
    #[derive(Debug, Clone)]
    pub struct AutomationsCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::AutomationRequest, String>,
    }
    impl<'a> AutomationsCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::AutomationRequest>,
            <V as std::convert::TryInto<types::AutomationRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `AutomationRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::AutomationRequest,
                ) -> types::builder::AutomationRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/automations/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::AutomationReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::AutomationRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/automations/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "automations_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::calendar_state`]

    [`Client::calendar_state`]: super::Client::calendar_state*/
    #[derive(Debug, Clone)]
    pub struct CalendarState<'a> {
        client: &'a super::Client,
        from: Result<Option<i64>, String>,
        to: Result<Option<i64>, String>,
    }
    impl<'a> CalendarState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                from: Ok(None),
                to: Ok(None),
            }
        }
        pub fn from<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<i64>,
        {
            self.from = value
                .try_into()
                .map(Some)
                .map_err(|_| "conversion to `i64` for from failed".to_string());
            self
        }
        pub fn to<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<i64>,
        {
            self.to = value
                .try_into()
                .map(Some)
                .map_err(|_| "conversion to `i64` for to failed".to_string());
            self
        }
        ///Sends a `GET` request to `/v1/calendar`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::CalendarSnapshot>, Error<types::ErrorBody>> {
            let Self { client, from, to } = self;
            let from = from.map_err(Error::InvalidRequest)?;
            let to = to.map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/calendar", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .query(&progenitor_client::QueryParam::new("from", &from))
                .query(&progenitor_client::QueryParam::new("to", &to))
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "calendar_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::calendar_command`]

    [`Client::calendar_command`]: super::Client::calendar_command*/
    #[derive(Debug, Clone)]
    pub struct CalendarCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::CalendarRequest, String>,
    }
    impl<'a> CalendarCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::CalendarRequest>,
            <V as std::convert::TryInto<types::CalendarRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `CalendarRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::CalendarRequest) -> types::builder::CalendarRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/calendar/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::CalendarReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::CalendarRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/calendar/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "calendar_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::calendar_import`]

    [`Client::calendar_import`]: super::Client::calendar_import*/
    #[derive(Debug, Clone)]
    pub struct CalendarImport<'a> {
        client: &'a super::Client,
        body: Result<types::builder::CalendarImportRequest, String>,
    }
    impl<'a> CalendarImport<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::CalendarImportRequest>,
            <V as std::convert::TryInto<types::CalendarImportRequest>>::Error: std::fmt::Display,
        {
            self.body = value.try_into().map(From::from).map_err(|s| {
                format!(
                    "conversion to `CalendarImportRequest` for body failed: {}",
                    s
                )
            });
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::CalendarImportRequest,
                ) -> types::builder::CalendarImportRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/calendar/imports`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::CalendarReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::CalendarImportRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/calendar/imports", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "calendar_import",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::product_command`]

    [`Client::product_command`]: super::Client::product_command*/
    #[derive(Debug, Clone)]
    pub struct ProductCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::CommandRequest, String>,
    }
    impl<'a> ProductCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::CommandRequest>,
            <V as std::convert::TryInto<types::CommandRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `CommandRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::CommandRequest) -> types::builder::CommandRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::Acknowledgement>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::CommandRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "product_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::connection_status`]

    [`Client::connection_status`]: super::Client::connection_status*/
    #[derive(Debug, Clone)]
    pub struct ConnectionStatus<'a> {
        client: &'a super::Client,
    }
    impl<'a> ConnectionStatus<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/connection`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::ConnectionStatus>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/connection", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "connection_status",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::connection_cancel`]

    [`Client::connection_cancel`]: super::Client::connection_cancel*/
    #[derive(Debug, Clone)]
    pub struct ConnectionCancel<'a> {
        client: &'a super::Client,
    }
    impl<'a> ConnectionCancel<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `POST` request to `/v1/connection/cancel`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::ConnectionStatus>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/connection/cancel", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "connection_cancel",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::connection_login`]

    [`Client::connection_login`]: super::Client::connection_login*/
    #[derive(Debug, Clone)]
    pub struct ConnectionLogin<'a> {
        client: &'a super::Client,
    }
    impl<'a> ConnectionLogin<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `POST` request to `/v1/connection/login`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::ConnectionStatus>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/connection/login", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "connection_login",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::connection_logout`]

    [`Client::connection_logout`]: super::Client::connection_logout*/
    #[derive(Debug, Clone)]
    pub struct ConnectionLogout<'a> {
        client: &'a super::Client,
    }
    impl<'a> ConnectionLogout<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `POST` request to `/v1/connection/logout`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::ConnectionStatus>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/connection/logout", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "connection_logout",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::home_state`]

    [`Client::home_state`]: super::Client::home_state*/
    #[derive(Debug, Clone)]
    pub struct HomeState<'a> {
        client: &'a super::Client,
    }
    impl<'a> HomeState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/home`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::HomeSnapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/home", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "home_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::home_command`]

    [`Client::home_command`]: super::Client::home_command*/
    #[derive(Debug, Clone)]
    pub struct HomeCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::HomeRequest, String>,
    }
    impl<'a> HomeCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::HomeRequest>,
            <V as std::convert::TryInto<types::HomeRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `HomeRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::HomeRequest) -> types::builder::HomeRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/home/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::HomeReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::HomeRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/home/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "home_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::home_connect`]

    [`Client::home_connect`]: super::Client::home_connect*/
    #[derive(Debug, Clone)]
    pub struct HomeConnect<'a> {
        client: &'a super::Client,
        body: Result<types::builder::HomeConnectionRequest, String>,
    }
    impl<'a> HomeConnect<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::HomeConnectionRequest>,
            <V as std::convert::TryInto<types::HomeConnectionRequest>>::Error: std::fmt::Display,
        {
            self.body = value.try_into().map(From::from).map_err(|s| {
                format!(
                    "conversion to `HomeConnectionRequest` for body failed: {}",
                    s
                )
            });
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::HomeConnectionRequest,
                ) -> types::builder::HomeConnectionRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `PUT` request to `/v1/home/connection`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::HomeConnection>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::HomeConnectionRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/home/connection", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .put(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "home_connect",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::home_disconnect`]

    [`Client::home_disconnect`]: super::Client::home_disconnect*/
    #[derive(Debug, Clone)]
    pub struct HomeDisconnect<'a> {
        client: &'a super::Client,
    }
    impl<'a> HomeDisconnect<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `DELETE` request to `/v1/home/connection`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/home/connection", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .delete(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "home_disconnect",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::product_state`]

    [`Client::product_state`]: super::Client::product_state*/
    #[derive(Debug, Clone)]
    pub struct ProductState<'a> {
        client: &'a super::Client,
    }
    impl<'a> ProductState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/state`
        pub async fn send(self) -> Result<ResponseValue<types::Snapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/state", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "product_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::temporal_executions`]

    [`Client::temporal_executions`]: super::Client::temporal_executions*/
    #[derive(Debug, Clone)]
    pub struct TemporalExecutions<'a> {
        client: &'a super::Client,
        page: Result<Option<::std::string::String>, String>,
        status: Result<Option<::std::string::String>, String>,
    }
    impl<'a> TemporalExecutions<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                page: Ok(None),
                status: Ok(None),
            }
        }
        pub fn page<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<::std::string::String>,
        {
            self.page = value.try_into().map(Some).map_err(|_| {
                "conversion to `:: std :: string :: String` for page failed".to_string()
            });
            self
        }
        pub fn status<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<::std::string::String>,
        {
            self.status = value.try_into().map(Some).map_err(|_| {
                "conversion to `:: std :: string :: String` for status failed".to_string()
            });
            self
        }
        ///Sends a `GET` request to `/v1/temporal/executions`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::ExecutionPage>, Error<types::ErrorBody>> {
            let Self {
                client,
                page,
                status,
            } = self;
            let page = page.map_err(Error::InvalidRequest)?;
            let status = status.map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/temporal/executions", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .query(&progenitor_client::QueryParam::new("page", &page))
                .query(&progenitor_client::QueryParam::new("status", &status))
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "temporal_executions",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::terminal_sessions_list`]

    [`Client::terminal_sessions_list`]: super::Client::terminal_sessions_list*/
    #[derive(Debug, Clone)]
    pub struct TerminalSessionsList<'a> {
        client: &'a super::Client,
    }
    impl<'a> TerminalSessionsList<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/terminal/sessions`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<::std::vec::Vec<types::TerminalSession>>, Error<types::ErrorBody>>
        {
            let Self { client } = self;
            let url = format!("{}/v1/terminal/sessions", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "terminal_sessions_list",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::terminal_sessions_create`]

    [`Client::terminal_sessions_create`]: super::Client::terminal_sessions_create*/
    #[derive(Debug, Clone)]
    pub struct TerminalSessionsCreate<'a> {
        client: &'a super::Client,
        body: Result<types::builder::CreateTerminalSession, String>,
    }
    impl<'a> TerminalSessionsCreate<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::CreateTerminalSession>,
            <V as std::convert::TryInto<types::CreateTerminalSession>>::Error: std::fmt::Display,
        {
            self.body = value.try_into().map(From::from).map_err(|s| {
                format!(
                    "conversion to `CreateTerminalSession` for body failed: {}",
                    s
                )
            });
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::CreateTerminalSession,
                ) -> types::builder::CreateTerminalSession,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/terminal/sessions`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::TerminalSession>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::CreateTerminalSession::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/terminal/sessions", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "terminal_sessions_create",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::terminal_sessions_close`]

    [`Client::terminal_sessions_close`]: super::Client::terminal_sessions_close*/
    #[derive(Debug, Clone)]
    pub struct TerminalSessionsClose<'a> {
        client: &'a super::Client,
        id: Result<::std::string::String, String>,
    }
    impl<'a> TerminalSessionsClose<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                id: Err("id was not initialized".to_string()),
            }
        }
        pub fn id<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<::std::string::String>,
        {
            self.id = value.try_into().map_err(|_| {
                "conversion to `:: std :: string :: String` for id failed".to_string()
            });
            self
        }
        ///Sends a `DELETE` request to `/v1/terminal/sessions/{id}`
        pub async fn send(self) -> Result<ResponseValue<()>, Error<types::ErrorBody>> {
            let Self { client, id } = self;
            let id = id.map_err(Error::InvalidRequest)?;
            let url = format!(
                "{}/v1/terminal/sessions/{}",
                client.baseurl,
                encode_path(&id.to_string()),
            );
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .delete(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "terminal_sessions_close",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                204u16 => Ok(ResponseValue::empty(response)),
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                404u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::tickets_state`]

    [`Client::tickets_state`]: super::Client::tickets_state*/
    #[derive(Debug, Clone)]
    pub struct TicketsState<'a> {
        client: &'a super::Client,
    }
    impl<'a> TicketsState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/tickets`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::TicketSnapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/tickets", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "tickets_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                403u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::tickets_command`]

    [`Client::tickets_command`]: super::Client::tickets_command*/
    #[derive(Debug, Clone)]
    pub struct TicketsCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::TicketCommandRequest, String>,
    }
    impl<'a> TicketsCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TicketCommandRequest>,
            <V as std::convert::TryInto<types::TicketCommandRequest>>::Error: std::fmt::Display,
        {
            self.body = value.try_into().map(From::from).map_err(|s| {
                format!(
                    "conversion to `TicketCommandRequest` for body failed: {}",
                    s
                )
            });
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::TicketCommandRequest,
                ) -> types::builder::TicketCommandRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/tickets/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::TicketReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::TicketCommandRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/tickets/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "tickets_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                403u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::ticket_contract`]

    [`Client::ticket_contract`]: super::Client::ticket_contract*/
    #[derive(Debug, Clone)]
    pub struct TicketContract<'a> {
        client: &'a super::Client,
        body: Result<types::builder::TicketContract, String>,
    }
    impl<'a> TicketContract<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TicketContract>,
            <V as std::convert::TryInto<types::TicketContract>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `TicketContract` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::TicketContract) -> types::builder::TicketContract,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/tickets/contract`
        pub async fn send(self) -> Result<ResponseValue<types::TicketContract>, Error<()>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::TicketContract::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/tickets/contract", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "ticket_contract",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::workspaces_state`]

    [`Client::workspaces_state`]: super::Client::workspaces_state*/
    #[derive(Debug, Clone)]
    pub struct WorkspacesState<'a> {
        client: &'a super::Client,
    }
    impl<'a> WorkspacesState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/workspaces`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::WorkspaceState>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/workspaces", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "workspaces_state",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::workspaces_command`]

    [`Client::workspaces_command`]: super::Client::workspaces_command*/
    #[derive(Debug, Clone)]
    pub struct WorkspacesCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::WorkspaceRequest, String>,
    }
    impl<'a> WorkspacesCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::WorkspaceRequest>,
            <V as std::convert::TryInto<types::WorkspaceRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `WorkspaceRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(
                    types::builder::WorkspaceRequest,
                ) -> types::builder::WorkspaceRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/workspaces/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::WorkspaceReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::WorkspaceRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/workspaces/commands", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .post(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .json(&body)
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "workspaces_command",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                400u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                503u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::get_version`]

    [`Client::get_version`]: super::Client::get_version*/
    #[derive(Debug, Clone)]
    pub struct GetVersion<'a> {
        client: &'a super::Client,
    }
    impl<'a> GetVersion<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/version`
        pub async fn send(self) -> Result<ResponseValue<types::Version>, Error<()>> {
            let Self { client } = self;
            let url = format!("{}/version", client.baseurl,);
            let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
            header_map.append(
                ::reqwest::header::HeaderName::from_static("api-version"),
                ::reqwest::header::HeaderValue::from_static(super::Client::api_version()),
            );
            #[allow(unused_mut)]
            let mut request = client
                .client
                .get(url)
                .header(
                    ::reqwest::header::ACCEPT,
                    ::reqwest::header::HeaderValue::from_static("application/json"),
                )
                .headers(header_map)
                .build()?;
            let info = OperationInfo {
                operation_id: "get_version",
            };
            match (crate::client_header)(&mut request).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            client.pre(&mut request, &info).await?;
            let result = client.exec(request, &info).await;
            client.post(&result, &info).await?;
            match (crate::server_compatibility)(&result).await {
                Ok(_) => {}
                Err(e) => return Err(Error::Custom(e.to_string())),
            }
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
}
/// Items consumers will typically use such as the Client.
pub mod prelude {
    pub use self::super::Client;
}
