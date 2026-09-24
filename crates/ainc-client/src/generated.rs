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
