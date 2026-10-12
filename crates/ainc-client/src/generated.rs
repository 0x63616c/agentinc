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
    ///        "ends_at",
    ///        "kind",
    ///        "starts_at",
    ///        "title"
    ///      ],
    ///      "properties": {
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
    ///            "delete"
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
    pub enum CalendarCommand {
        #[serde(rename = "create")]
        Create {
            ends_at: i64,
            starts_at: i64,
            title: ::std::string::String,
        },
        #[serde(rename = "delete")]
        Delete { id: i64 },
    }
    ///Times are epoch seconds.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Times are epoch seconds.",
    ///  "type": "object",
    ///  "required": [
    ///    "ends_at",
    ///    "id",
    ///    "starts_at",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "ends_at": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
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
        pub ends_at: i64,
        pub id: i64,
        pub starts_at: i64,
        pub title: ::std::string::String,
    }
    impl CalendarEvent {
        pub fn builder() -> builder::CalendarEvent {
            Default::default()
        }
    }
    ///The id of the Event the command touched.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The id of the Event the command touched.",
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarReceipt {
        pub result_id: i64,
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
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct CalendarSnapshot {
        pub events: ::std::vec::Vec<CalendarEvent>,
    }
    impl CalendarSnapshot {
        pub fn builder() -> builder::CalendarSnapshot {
            Default::default()
        }
    }
    ///The one shape every non-2xx response has.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The one shape every non-2xx response has.",
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "message"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "$ref": "#/components/schemas/ErrorCode"
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
        pub code: ErrorCode,
        pub message: ::std::string::String,
    }
    impl ErrorBody {
        pub fn builder() -> builder::ErrorBody {
            Default::default()
        }
    }
    ///The closed set of error codes a client can switch on.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The closed set of error codes a client can switch on.",
    ///  "type": "string",
    ///  "enum": [
    ///    "invalid",
    ///    "conflict",
    ///    "unauthorized",
    ///    "forbidden",
    ///    "not_found",
    ///    "unavailable",
    ///    "internal",
    ///    "upgrade_required"
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
    pub enum ErrorCode {
        #[serde(rename = "invalid")]
        Invalid,
        #[serde(rename = "conflict")]
        Conflict,
        #[serde(rename = "unauthorized")]
        Unauthorized,
        #[serde(rename = "forbidden")]
        Forbidden,
        #[serde(rename = "not_found")]
        NotFound,
        #[serde(rename = "unavailable")]
        Unavailable,
        #[serde(rename = "internal")]
        Internal,
        #[serde(rename = "upgrade_required")]
        UpgradeRequired,
    }
    impl ::std::fmt::Display for ErrorCode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Invalid => f.write_str("invalid"),
                Self::Conflict => f.write_str("conflict"),
                Self::Unauthorized => f.write_str("unauthorized"),
                Self::Forbidden => f.write_str("forbidden"),
                Self::NotFound => f.write_str("not_found"),
                Self::Unavailable => f.write_str("unavailable"),
                Self::Internal => f.write_str("internal"),
                Self::UpgradeRequired => f.write_str("upgrade_required"),
            }
        }
    }
    impl ::std::str::FromStr for ErrorCode {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "invalid" => Ok(Self::Invalid),
                "conflict" => Ok(Self::Conflict),
                "unauthorized" => Ok(Self::Unauthorized),
                "forbidden" => Ok(Self::Forbidden),
                "not_found" => Ok(Self::NotFound),
                "unavailable" => Ok(Self::Unavailable),
                "internal" => Ok(Self::Internal),
                "upgrade_required" => Ok(Self::UpgradeRequired),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ErrorCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
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
    ///`TodoCommand`
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
    ///            "add"
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
    ///            "complete"
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
    ///            "delete"
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
    pub enum TodoCommand {
        #[serde(rename = "add")]
        Add { title: ::std::string::String },
        #[serde(rename = "complete")]
        Complete { completed: bool, id: i64 },
        #[serde(rename = "delete")]
        Delete { id: i64 },
    }
    ///The id of the Todo the command touched.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "The id of the Todo the command touched.",
    ///  "type": "object",
    ///  "required": [
    ///    "result_id"
    ///  ],
    ///  "properties": {
    ///    "result_id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TodoReceipt {
        pub result_id: i64,
    }
    impl TodoReceipt {
        pub fn builder() -> builder::TodoReceipt {
            Default::default()
        }
    }
    ///`TodoRequest`
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
    ///      "$ref": "#/components/schemas/TodoCommand"
    ///    },
    ///    "operation_id": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TodoRequest {
        pub command: TodoCommand,
        pub operation_id: ::std::string::String,
    }
    impl TodoRequest {
        pub fn builder() -> builder::TodoRequest {
            Default::default()
        }
    }
    ///`TodoSnapshot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "todos"
    ///  ],
    ///  "properties": {
    ///    "todos": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Todo"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, schemars::JsonSchema)]
    pub struct TodoSnapshot {
        pub todos: ::std::vec::Vec<Todo>,
    }
    impl TodoSnapshot {
        pub fn builder() -> builder::TodoSnapshot {
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
    ///`WorkspaceSnapshot`
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
    pub struct WorkspaceSnapshot {
        pub current_id: ::std::string::String,
        pub workspaces: ::std::vec::Vec<Workspace>,
    }
    impl WorkspaceSnapshot {
        pub fn builder() -> builder::WorkspaceSnapshot {
            Default::default()
        }
    }
    /// Types for composing complex structures.
    pub mod builder {
        #[derive(Clone, Debug)]
        pub struct CalendarEvent {
            ends_at: ::std::result::Result<i64, ::std::string::String>,
            id: ::std::result::Result<i64, ::std::string::String>,
            starts_at: ::std::result::Result<i64, ::std::string::String>,
            title: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for CalendarEvent {
            fn default() -> Self {
                Self {
                    ends_at: Err("no value supplied for ends_at".to_string()),
                    id: Err("no value supplied for id".to_string()),
                    starts_at: Err("no value supplied for starts_at".to_string()),
                    title: Err("no value supplied for title".to_string()),
                }
            }
        }
        impl CalendarEvent {
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
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for id: {e}"));
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
                    ends_at: value.ends_at?,
                    id: value.id?,
                    starts_at: value.starts_at?,
                    title: value.title?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarEvent> for CalendarEvent {
            fn from(value: super::CalendarEvent) -> Self {
                Self {
                    ends_at: Ok(value.ends_at),
                    id: Ok(value.id),
                    starts_at: Ok(value.starts_at),
                    title: Ok(value.title),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct CalendarReceipt {
            result_id: ::std::result::Result<i64, ::std::string::String>,
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
                T: ::std::convert::TryInto<i64>,
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
        }
        impl ::std::default::Default for CalendarSnapshot {
            fn default() -> Self {
                Self {
                    events: Err("no value supplied for events".to_string()),
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
        }
        impl ::std::convert::TryFrom<CalendarSnapshot> for super::CalendarSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: CalendarSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    events: value.events?,
                })
            }
        }
        impl ::std::convert::From<super::CalendarSnapshot> for CalendarSnapshot {
            fn from(value: super::CalendarSnapshot) -> Self {
                Self {
                    events: Ok(value.events),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct ErrorBody {
            code: ::std::result::Result<super::ErrorCode, ::std::string::String>,
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
                T: ::std::convert::TryInto<super::ErrorCode>,
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
        pub struct TodoReceipt {
            result_id: ::std::result::Result<i64, ::std::string::String>,
        }
        impl ::std::default::Default for TodoReceipt {
            fn default() -> Self {
                Self {
                    result_id: Err("no value supplied for result_id".to_string()),
                }
            }
        }
        impl TodoReceipt {
            pub fn result_id<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<i64>,
                T::Error: ::std::fmt::Display,
            {
                self.result_id = value
                    .try_into()
                    .map_err(|e| format!("error converting supplied value for result_id: {e}"));
                self
            }
        }
        impl ::std::convert::TryFrom<TodoReceipt> for super::TodoReceipt {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TodoReceipt,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    result_id: value.result_id?,
                })
            }
        }
        impl ::std::convert::From<super::TodoReceipt> for TodoReceipt {
            fn from(value: super::TodoReceipt) -> Self {
                Self {
                    result_id: Ok(value.result_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TodoRequest {
            command: ::std::result::Result<super::TodoCommand, ::std::string::String>,
            operation_id: ::std::result::Result<::std::string::String, ::std::string::String>,
        }
        impl ::std::default::Default for TodoRequest {
            fn default() -> Self {
                Self {
                    command: Err("no value supplied for command".to_string()),
                    operation_id: Err("no value supplied for operation_id".to_string()),
                }
            }
        }
        impl TodoRequest {
            pub fn command<T>(mut self, value: T) -> Self
            where
                T: ::std::convert::TryInto<super::TodoCommand>,
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
        impl ::std::convert::TryFrom<TodoRequest> for super::TodoRequest {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TodoRequest,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    command: value.command?,
                    operation_id: value.operation_id?,
                })
            }
        }
        impl ::std::convert::From<super::TodoRequest> for TodoRequest {
            fn from(value: super::TodoRequest) -> Self {
                Self {
                    command: Ok(value.command),
                    operation_id: Ok(value.operation_id),
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct TodoSnapshot {
            todos: ::std::result::Result<::std::vec::Vec<super::Todo>, ::std::string::String>,
        }
        impl ::std::default::Default for TodoSnapshot {
            fn default() -> Self {
                Self {
                    todos: Err("no value supplied for todos".to_string()),
                }
            }
        }
        impl TodoSnapshot {
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
        }
        impl ::std::convert::TryFrom<TodoSnapshot> for super::TodoSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: TodoSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    todos: value.todos?,
                })
            }
        }
        impl ::std::convert::From<super::TodoSnapshot> for TodoSnapshot {
            fn from(value: super::TodoSnapshot) -> Self {
                Self {
                    todos: Ok(value.todos),
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
        pub struct WorkspaceSnapshot {
            current_id: ::std::result::Result<::std::string::String, ::std::string::String>,
            workspaces:
                ::std::result::Result<::std::vec::Vec<super::Workspace>, ::std::string::String>,
        }
        impl ::std::default::Default for WorkspaceSnapshot {
            fn default() -> Self {
                Self {
                    current_id: Err("no value supplied for current_id".to_string()),
                    workspaces: Err("no value supplied for workspaces".to_string()),
                }
            }
        }
        impl WorkspaceSnapshot {
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
        impl ::std::convert::TryFrom<WorkspaceSnapshot> for super::WorkspaceSnapshot {
            type Error = super::error::ConversionError;
            fn try_from(
                value: WorkspaceSnapshot,
            ) -> ::std::result::Result<Self, super::error::ConversionError> {
                Ok(Self {
                    current_id: value.current_id?,
                    workspaces: value.workspaces?,
                })
            }
        }
        impl ::std::convert::From<super::WorkspaceSnapshot> for WorkspaceSnapshot {
            fn from(value: super::WorkspaceSnapshot) -> Self {
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



Version: 0.8.0*/
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
        "0.8.0"
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
    /*Sends a `GET` request to `/v1/calendar`

    ```ignore
    let response = client.calendar_state()
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
    /*Sends a `GET` request to `/v1/todos`

    ```ignore
    let response = client.todos_state()
        .send()
        .await;
    ```*/
    pub fn todos_state(&self) -> builder::TodosState<'_> {
        builder::TodosState::new(self)
    }
    /*Sends a `POST` request to `/v1/todos/commands`

    ```ignore
    let response = client.todos_command()
        .body(body)
        .send()
        .await;
    ```*/
    pub fn todos_command(&self) -> builder::TodosCommand<'_> {
        builder::TodosCommand::new(self)
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
    let response = client.version_show()
        .send()
        .await;
    ```*/
    pub fn version_show(&self) -> builder::VersionShow<'_> {
        builder::VersionShow::new(self)
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
        pub async fn send(self) -> Result<ResponseValue<types::Health>, Error<types::ErrorBody>> {
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
    }
    impl<'a> CalendarState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/calendar`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::CalendarSnapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
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
                401u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
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
                404u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::todos_state`]

    [`Client::todos_state`]: super::Client::todos_state*/
    #[derive(Debug, Clone)]
    pub struct TodosState<'a> {
        client: &'a super::Client,
    }
    impl<'a> TodosState<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self { client: client }
        }
        ///Sends a `GET` request to `/v1/todos`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::TodoSnapshot>, Error<types::ErrorBody>> {
            let Self { client } = self;
            let url = format!("{}/v1/todos", client.baseurl,);
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
                operation_id: "todos_state",
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
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::todos_command`]

    [`Client::todos_command`]: super::Client::todos_command*/
    #[derive(Debug, Clone)]
    pub struct TodosCommand<'a> {
        client: &'a super::Client,
        body: Result<types::builder::TodoRequest, String>,
    }
    impl<'a> TodosCommand<'a> {
        pub fn new(client: &'a super::Client) -> Self {
            Self {
                client: client,
                body: Ok(::std::default::Default::default()),
            }
        }
        pub fn body<V>(mut self, value: V) -> Self
        where
            V: std::convert::TryInto<types::TodoRequest>,
            <V as std::convert::TryInto<types::TodoRequest>>::Error: std::fmt::Display,
        {
            self.body = value
                .try_into()
                .map(From::from)
                .map_err(|s| format!("conversion to `TodoRequest` for body failed: {}", s));
            self
        }
        pub fn body_map<F>(mut self, f: F) -> Self
        where
            F: std::ops::FnOnce(types::builder::TodoRequest) -> types::builder::TodoRequest,
        {
            self.body = self.body.map(f);
            self
        }
        ///Sends a `POST` request to `/v1/todos/commands`
        pub async fn send(
            self,
        ) -> Result<ResponseValue<types::TodoReceipt>, Error<types::ErrorBody>> {
            let Self { client, body } = self;
            let body = body
                .and_then(|v| types::TodoRequest::try_from(v).map_err(|e| e.to_string()))
                .map_err(Error::InvalidRequest)?;
            let url = format!("{}/v1/todos/commands", client.baseurl,);
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
                operation_id: "todos_command",
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
                404u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
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
        ) -> Result<ResponseValue<types::WorkspaceSnapshot>, Error<types::ErrorBody>> {
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
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
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
                404u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                409u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                426u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                500u16 => Err(Error::ErrorResponse(
                    ResponseValue::from_response(response).await?,
                )),
                _ => Err(Error::UnexpectedResponse(response)),
            }
        }
    }
    /*Builder for [`Client::version_show`]

    [`Client::version_show`]: super::Client::version_show*/
    #[derive(Debug, Clone)]
    pub struct VersionShow<'a> {
        client: &'a super::Client,
    }
    impl<'a> VersionShow<'a> {
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
                operation_id: "version_show",
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
