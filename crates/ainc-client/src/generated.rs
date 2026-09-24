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
        #[serde(rename = "create")]
        Create { title: ::std::string::String },
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
    }
}
#[derive(Clone, Debug)]
/*Client for ainc-daemon



Version: 0.1.0*/
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
        "0.1.0"
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
    /*Sends a `GET` request to `/v1/state`

    ```ignore
    let response = client.product_state()
        .send()
        .await;
    ```*/
    pub fn product_state(&self) -> builder::ProductState<'_> {
        builder::ProductState::new(self)
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
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
                503u16 => Err(Error::ErrorResponse(ResponseValue::empty(response))),
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
            let response = result?;
            match response.status().as_u16() {
                200u16 => ResponseValue::from_response(response).await,
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
