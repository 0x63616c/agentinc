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
