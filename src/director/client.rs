use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use url::Url;

use super::pow::{self, Solution};
use super::{
    errors::{
        ConflictError,
        ForbiddenError,
        InvalidRequestError,
        NotFoundError,
        PassphraseRequiredError,
        RateLimitError,
        RegistrationDisabledError,
        ServerError,
        ServiceBusyError,
        UnauthorizedError,
    },
    base64url, sign_nonce,
    Device,
    NodeStatus,
    UserRegistration,
    Avatar,
    Plan, UserPlan,
    Profile,
    ProfileUpdate,
    Subscription,
};
use crate::{
    errors::{ArgumentError, MalformedError, NetworkError, Result, StateError},
    signature::KeyPair,
    Id,
};

const API_PREFIX: &str = "api/v1/client";
const AUTH_NONCE_BYTES: usize = 32;

#[derive(Clone)]
pub struct DirectorClientBuilder {
    director_url: Option<Url>,
    node_id: Option<Id>,
    user_key: Option<KeyPair>,
    user_id: Option<Id>,
    device_key: Option<KeyPair>,
}

impl DirectorClientBuilder {
    pub fn new() -> Self {
        Self {
            director_url: None,
            node_id: None,
            user_key: None,
            user_id: None,
            device_key: None,
        }
    }

    pub fn with_director_url(&mut self, url: impl AsRef<str>) -> Result<&mut Self> {
        let url = Url::parse(url.as_ref()).map_err(|e| InvalidRequestError::new(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(InvalidRequestError::new(
                "Director URL must use http or https",
            ));
        }
        self.director_url = Some(url);
        Ok(self)
    }
    pub fn with_node_id(&mut self, node_id: Id) -> &mut Self {
        self.node_id = Some(node_id);
        self
    }

    pub fn with_user_key(&mut self, key: KeyPair) -> &mut Self {
        self.user_id = Some(Id::from(key.public_key()));
        self.user_key = Some(key);
        self
    }

    pub fn with_user_id(&mut self, user_id: Id) -> &mut Self {
        self.user_key = None;
        self.user_id = Some(user_id);
        self
    }

    pub fn with_device_key(&mut self, key: KeyPair) -> &mut Self {
        self.device_key = Some(key);
        self
    }

    pub fn build(&self) -> Result<DirectorClient> {
        let director_url = self
            .director_url
            .clone()
            .ok_or_else(|| ArgumentError::new("Director URL is required"))?;

        if self.user_id.is_none() && self.device_key.is_some() {
            return Err(ArgumentError::new(
                "A device key requires a user key or user ID",
            ));
        }
        if self.user_id.is_some() && self.user_key.is_none() && self.device_key.is_none() {
            return Err(ArgumentError::new("A user ID requires a device key"));
        }

        DirectorClient::new(
            director_url,
            self.node_id,
            self.user_key.clone(),
            self.user_id,
            self.device_key.clone(),
        )
    }
}

impl std::fmt::Debug for DirectorClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectorClientBuilder")
            .field("director_url", &self.director_url)
            .field("node_id", &self.node_id)
            .field("user_id", &self.user_id)
            .field("has_user_key", &self.user_key.is_some())
            .field("has_device_key", &self.device_key.is_some())
            .finish()
    }
}

impl Default for DirectorClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DirectorClient {
    client: Client,
    base_url: Url,
    node_id: Option<Id>,
    user_key: Option<KeyPair>,
    user_id: Option<Id>,
    device_key: Option<KeyPair>,
    device_id: Option<Id>,
    token: Mutex<Option<String>>,
    closed: AtomicBool,
}

impl std::fmt::Debug for DirectorClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectorClient")
            .field("base_url", &self.base_url)
            .field("node_id", &self.node_id)
            .field("user_id", &self.user_id)
            .field("device_id", &self.device_id)
            .field("is_closed", &self.is_closed())
            .finish()
    }
}

impl DirectorClient {
    pub fn builder() -> DirectorClientBuilder {
        DirectorClientBuilder::new()
    }

    fn new(
        director_url: Url,
        node_id: Option<Id>,
        user_key: Option<KeyPair>,
        user_id: Option<Id>,
        device_key: Option<KeyPair>,
    ) -> Result<Self> {
        let mut base_url = director_url;
        let path = base_url.path().trim_end_matches('/');
        let path = if path.ends_with(API_PREFIX) {
            format!("{path}/")
        } else {
            format!("{path}/{API_PREFIX}/")
        };
        base_url.set_path(&path);
        base_url.set_query(None);
        base_url.set_fragment(None);

        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| NetworkError::new(e.to_string()))?;
        let device_id = device_key.as_ref().map(|key| Id::from(key.public_key()));

        Ok(Self {
            client,
            base_url,
            node_id,
            user_key,
            user_id,
            device_key,
            device_id,
            token: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }

    pub fn director_url(&self) -> &Url {
        &self.base_url
    }

    pub fn node_id(&self) -> Option<&Id> {
        self.node_id.as_ref()
    }

    pub fn user_id(&self) -> Option<&Id> {
        self.user_id.as_ref()
    }

    pub fn device_id(&self) -> Option<&Id> {
        self.device_id.as_ref()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.clear_token();
    }

    fn get_token(&self) -> Option<String> {
        match self.token.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn set_token(&self, token: Option<String>) {
        match self.token.lock() {
            Ok(mut guard) => *guard = token,
            Err(poisoned) => *poisoned.into_inner() = token,
        }
    }

    fn clear_token(&self) {
        self.set_token(None);
    }

    pub async fn get_node_id(&self) -> Result<Id> {
        self.check_open()?;
        let body = self.http_get("id", false).await?;
        let id = body
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| MalformedError::new("missing 'id'"))?;

        Id::try_from_base58(id).map_err(|e| {
            MalformedError::new(e.to_string()).into()
        })
    }

    pub async fn get_node_status(&self) -> Result<NodeStatus> {
        self.check_open()?;
        self.http_get_json("node", false).await
    }

    pub async fn register_user(&self, registration: UserRegistration) -> Result<()> {
        self.check_open()?;

        let user_key = self
            .user_key
            .as_ref()
            .ok_or_else(|| StateError::new("Registering a user needs the user key"))?;

        let initial_device_key = if registration.has_initial_device() {
            Some(self.device_key.as_ref().ok_or_else(|| {
                StateError::new("Registering an initial device needs the device key")
            })?)
        } else {
            None
        };

        let node_id = match self.node_id {
            Some(node_id) => node_id,
            _ => self.get_node_id().await?,
        };
        let challenge = self.fetch_challenge().await?;
        let solve_key = user_key.clone();
        let challenge_nonce = challenge.nonce;
        let challenge_n = challenge.n;
        let challenge_k = challenge.k;
        let challenge_effort = challenge.effort;
        let solution = tokio::task::spawn_blocking(move || {
            pow::solve(
                node_id,
                &solve_key,
                challenge_n,
                challenge_k,
                challenge_effort,
                &challenge_nonce,
            )
            .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| StateError::new(format!("Registration solver failed: {error}")))?
        .map_err(|error| StateError::new(error))?;

        self.submit_registration(
            &registration,
            node_id,
            user_key,
            initial_device_key,
            &challenge,
            solution,
        ).await
    }

    pub async fn register_device(
        &self,
        name: &str,
        app: &str,
        passphrase: Option<&str>,
    ) -> Result<()> {
        let key = self.device_key.as_ref().ok_or_else(|| {
            StateError::new("No device key configured; pass a device key to register")
        })?;
        self.register_device_with_key(key, name, app, passphrase)
            .await
    }

    pub async fn register_device_with_key(
        &self,
        key: &KeyPair,
        name: &str,
        app: &str,
        passphrase: Option<&str>,
    ) -> Result<()> {
        self.check_open()?;
        self.check_identity()?;

        if name.trim().is_empty() || app.trim().is_empty() {
            return Err(InvalidRequestError::new(
                "Device name and app name cannot be empty",
            ));
        }

        let nonce = crate::random_array::<AUTH_NONCE_BYTES>();
        let mut body = json!({
            "deviceId": Id::from(key.public_key()),
            "deviceName": name,
            "appName": app,
            "nonce": base64url(&nonce),
            "deviceSig": sign_nonce(key, &nonce).map_err(|e|
                MalformedError::new(e.to_string()))?
        });
        if let Some(passphrase) = passphrase {
            if passphrase.is_empty() {
                return Err(InvalidRequestError::new("Passphrase cannot be empty"));
            }
            body["passphrase"] = json!(passphrase);
        }

        self.http_call(Method::POST, "devices", Some(body), None, true)
            .await
            .map(|_| ())
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        self.check_open()?;
        self.check_identity()?;

        self.http_get_json("devices", true).await
    }

    pub async fn remove_device(&self, device_id: &Id, passphrase: Option<&str>) -> Result<()> {
        self.check_open()?;
        self.check_identity()?;

        let mut body = json!({});
        if let Some(passphrase) = passphrase {
            if passphrase.is_empty() {
                return Err(InvalidRequestError::new("Passphrase cannot be empty"));
            }
            body["passphrase"] = json!(passphrase);
        }
        self.http_call(
            Method::POST,
            &format!("devices/{}/remove", device_id.to_base58()),
            Some(body),
            None,
            true,
        )
        .await
        .map(|_| ())
    }

    pub async fn set_passphrase(&self, passphrase: &str) -> Result<()> {
        self.passphrase(Method::PUT, "passphrase", passphrase, None)
            .await
    }

    pub async fn update_passphrase(&self, current: &str, new: &str) -> Result<()> {
        self.passphrase(Method::PUT, "passphrase", new, Some(current))
            .await
    }

    pub async fn clear_passphrase(&self, current: &str) -> Result<()> {
        self.passphrase(Method::POST, "passphrase/clear", current, None)
            .await
    }

    pub async fn get_profile(&self) -> Result<Profile> {
        self.check_open()?;
        self.check_identity()?;

        self.http_get_json("profile", true).await
    }

    pub async fn update_profile(
        &self,
        update: &ProfileUpdate,
        passphrase: Option<&str>,
    ) -> Result<()> {
        self.check_open()?;
        self.check_identity()?;

        if update.is_empty() {
            return Err(InvalidRequestError::new(
                "The profile update changes nothing",
            ));
        }
        let mut body = Value::Object(update.fields());
        if let Some(passphrase) = passphrase {
            if passphrase.is_empty() {
                return Err(InvalidRequestError::new("Passphrase cannot be empty"));
            }
            body["passphrase"] = json!(passphrase);
        }
        self.http_call(Method::PUT, "profile", Some(body), None, true)
            .await
            .map(|_| ())
    }

    pub async fn update_avatar(&self, image: &[u8], content_type: &str) -> Result<String> {
        self.check_open()?;
        self.check_identity()?;

        if image.is_empty() {
            return Err(InvalidRequestError::new("The avatar image is empty"));
        }
        let content_type = avatar_content_type(content_type)?;
        let body = self
            .http_call(
                Method::PUT,
                "avatar",
                None,
                Some((image.to_vec(), content_type.to_owned())),
                true,
            )
            .await?;

        body.get("uri")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MalformedError::new("missing 'uri'").into())
    }

    pub async fn get_avatar(&self) -> Result<Option<Avatar>> {
        self.check_open()?;
        self.check_identity()?;
        match self.raw(Method::GET, "avatar", None, None, true).await {
            Ok(response)
                if response.status() == StatusCode::NOT_FOUND
                    || response.status() == StatusCode::NO_CONTENT =>
            {
                Ok(None)
            }
            Ok(response) => {
                let response = self.check_response(response).await?;
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                let data = response
                    .bytes()
                    .await
                    .map_err(|e| NetworkError::new(e.to_string()))?
                    .to_vec();
                Ok(Some(Avatar { content_type, data }))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn get_plan(&self) -> Result<UserPlan> {
        self.check_open()?;
        self.check_identity()?;

        let profile = self.get_profile().await?;
        let subscription_response = self
            .raw(Method::GET, "subscriptions/active", None, None, true)
            .await?;
        let subscription: Option<Subscription> = match subscription_response.status() {
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => None,
            _ => {
                let response = self.check_response(subscription_response).await?;
                let subscription: Subscription = response
                    .json()
                    .await
                    .map_err(|e| MalformedError::new(e.to_string()))?;
                Some(subscription)
            }
        };

        let plans: Vec<Plan> = self.http_get_json("plans", false).await?;
        let plan = plans.into_iter().find(|plan| {
            subscription
                .as_ref()
                .map_or(plan.name() == profile.plan_name(), |subscription| {
                    plan.id() == subscription.plan_id
                })
        });
        Ok(UserPlan {
            name: profile.plan_name().into(),
            plan,
            subscription,
        })
    }

    async fn passphrase(
        &self,
        method: Method,
        path: &str,
        passphrase: &str,
        current: Option<&str>,
    ) -> Result<()> {
        self.check_open()?;
        self.check_identity()?;

        if passphrase.is_empty() || current.is_some_and(str::is_empty) {
            return Err(InvalidRequestError::new("Passphrase cannot be empty"));
        }

        let mut body = json!({"passphrase": passphrase});
        if let Some(current) = current {
            body["currentPassphrase"] = json!(current);
        }

        self.http_call(method, path, Some(body), None, true)
            .await
            .map(|_| ())
    }

    async fn fetch_challenge(&self) -> Result<Challenge> {
        let body = match self
            .http_call(Method::GET, "users/challenge", None, None, false)
            .await
        {
            Ok(body) => body,
            Err(error) if error.downcast_ref::<NotFoundError>().is_some() => {
                return Err(RegistrationDisabledError::new(
                    404,
                    "This node does not accept proof-of-work registration; it registers users through OAuth only",
                ));
            }
            Err(error) => return Err(error),
        };
        Challenge::parse(body)
    }

    async fn submit_registration(
        &self,
        registration: &UserRegistration,
        node_id: Id,
        user_key: &KeyPair,
        initial_device_key: Option<&KeyPair>,
        challenge: &Challenge,
        solution: Solution,
    ) -> Result<()> {
        let mut body = json!({
            "userId": Id::from(user_key.public_key()),
            "challenge": base64url(&challenge.token),
            "challengeSig": base64url(&challenge.signature),
            "powNonce": base64url(&solution.pow_nonce),
            "solution": solution.indices,
            "userSig": base64url(&solution.signature),
        });
        insert_optional_string(&mut body, "userName", registration.name());
        insert_optional_string(&mut body, "email", registration.email());
        insert_optional_string(&mut body, "bio", registration.bio());
        insert_optional_string(&mut body, "passphrase", registration.passphrase());

        let path = if let Some(device_key) = initial_device_key {
            body["deviceId"] = json!(Id::from(device_key.public_key()));
            body["deviceName"] = json!(registration.device_name());
            body["appName"] = json!(registration.app_name());
            body["deviceSig"] = json!(base64url(
                &pow::sign(
                    node_id,
                    device_key,
                    &challenge.nonce,
                    &solution.pow_nonce,
                    challenge.effort,
                )
                .map_err(|error| MalformedError::new(error.to_string()))?,
            ));
            "usersAndInitialDevice"
        } else {
            "users"
        };
        let token = self
            .http_call(Method::POST, path, Some(body), None, false)
            .await?
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MalformedError::new("missing 'token'"))?;
        self.set_token(Some(token));
        Ok(())
    }

    fn check_open(&self) -> Result<()> {
        if self.is_closed() {
            Err(StateError::new("Client is closed"))
        } else {
            Ok(())
        }
    }
    fn check_identity(&self) -> Result<()> {
        if self.user_id.is_none() {
            Err(StateError::new("This call needs a user identity"))
        } else {
            Ok(())
        }
    }

    async fn http_get(&self, path: &str, authenticated: bool) -> Result<Value> {
        self.http_call(Method::GET, path, None, None, authenticated).await
    }

    async fn http_get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        authenticated: bool,
    ) -> Result<T> {
        let response = self.raw(Method::GET, path, None, None, authenticated).await?;
        let response = self.check_response(response).await?;
        response
            .json()
            .await
            .map_err(|e| MalformedError::new(e.to_string()).into())
    }

    async fn http_call(
        &self,
        method: Method,
        path: &str,
        json: Option<Value>,
        binary: Option<(Vec<u8>, String)>,
        authenticated: bool,
    ) -> Result<Value> {
        let response = self.raw(method, path, json, binary, authenticated).await?;
        let response = self.check_response(response).await?;
        response
            .json()
            .await
            .map_err(|e| MalformedError::new(e.to_string()).into())
    }
    async fn raw(
        &self,
        method: Method,
        path: &str,
        json: Option<Value>,
        binary: Option<(Vec<u8>, String)>,
        authenticated: bool,
    ) -> Result<reqwest::Response> {
        if self.is_closed() {
            return Err(StateError::new("Client is closed"));
        }
        let token = if authenticated {
            Some(self.access_token().await?)
        } else {
            None
        };
        let mut request = self.client.request(method.clone(), self.url(path)?);
        if let Some(token) = token.as_ref() {
            request = request.bearer_auth(token);
        }
        if let Some(json) = &json {
            request = request.json(json);
        }
        if let Some((body, content_type)) = &binary {
            request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body.clone());
        }
        let response = request.send().await.map_err(|e|
            NetworkError::new(e.to_string())
        )?;
        if authenticated && response.status() == StatusCode::UNAUTHORIZED {
            self.clear_token();
            let err_text = response.text().await.unwrap_or_default();
            let err_msg = parse_error_message(&err_text);
            return Err(UnauthorizedError::new(err_msg));
        }
        Ok(response)
    }
    async fn check_response(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_success() {
            Ok(response)
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            let message = parse_error_message(&text);
            Err(response_error(status, message))
        }
    }
    async fn access_token(&self) -> Result<String> {
        if let Some(token) = self.get_token() {
            return Ok(token);
        }
        let user_id = self
            .user_id
            .ok_or_else(|| StateError::new("This call needs a user identity"))?;
        let nonce = crate::random_array::<AUTH_NONCE_BYTES>();
        let mut body = json!({"userId": user_id, "nonce": base64url(&nonce)});
        if let Some(key) = &self.user_key {
            body["userSig"] =
                json!(sign_nonce(key, &nonce).map_err(|e|
                    MalformedError::new(e.to_string()))?);
        } else if let Some(key) = &self.device_key {
            body["deviceId"] = json!(self.device_id);
            body["deviceSig"] =
                json!(sign_nonce(key, &nonce).map_err(|e|
                    MalformedError::new(e.to_string()))?);
        } else {
            return Err(StateError::new("This call needs a user identity"));
        }
        let response = self
            .client
            .post(self.url("auth")?)
            .json(&body)
            .send()
            .await
            .map_err(|e| NetworkError::new(e.to_string()))?;
        let token = self
            .check_response(response)
            .await?
            .json::<Value>()
            .await
            .map_err(|e| MalformedError::new(e.to_string()))?
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| MalformedError::new("missing 'token'"))?;
        self.set_token(Some(token.clone()));
        Ok(token)
    }
    fn url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| NetworkError::new(e.to_string()).into())
    }
}

fn avatar_content_type(content_type: &str) -> Result<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/png" => Ok("image/png"),
        "image/jpeg" | "image/jpg" => Ok("image/jpeg"),
        _ => Err(InvalidRequestError::new(
            "Unsupported avatar type (PNG or JPEG only)",
        )),
    }
}

fn parse_error_message(text: &str) -> String {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(msg) = value.get("message").and_then(Value::as_str) {
            return msg.to_owned();
        }
        if let Some(err) = value.get("error").and_then(Value::as_str) {
            return err.to_owned();
        }
    }
    trimmed.to_owned()
}

fn response_error(status: u16, message: String) -> Box<dyn std::error::Error> {
    match status {
        400 => InvalidRequestError::new(message),
        401 => UnauthorizedError::new(message),
        403 => ForbiddenError::new(message),
        404 => NotFoundError::new(message),
        409 => ConflictError::new(message),
        428 => PassphraseRequiredError::new(message),
        429 => RateLimitError::new(message),
        503 => ServiceBusyError::new(message),
        _ => ServerError::new(status, message),
    }
}

struct Challenge {
    token: Vec<u8>,
    signature: Vec<u8>,
    nonce: [u8; 32],
    n: u32,
    k: u32,
    effort: u32,
}

impl Challenge {
    fn parse(body: Value) -> Result<Self> {
        use base64::Engine;

        let decode = |name: &str| {
            body.get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| MalformedError::new(format!("missing '{name}'")))
                .and_then(|value| {
                    let clean = value.trim_end_matches('=');
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(clean)
                        .map_err(|error| MalformedError::new(format!("invalid '{name}': {error}")))
                })
        };
        let nonce = decode("nonce")?;
        let nonce = nonce
            .try_into()
            .map_err(|_| MalformedError::new("'nonce' must contain 32 bytes"))?;
        let unsigned = |name: &str| {
            body.get(name)
                .and_then(Value::as_u64)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| MalformedError::new(format!("invalid '{name}'")))
        };
        Ok(Self {
            token: decode("challenge")?,
            signature: decode("challengeSig")?,
            nonce,
            n: unsigned("n")?,
            k: unsigned("k")?,
            effort: unsigned("effort")?,
        })
    }
}

fn insert_optional_string(body: &mut Value, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        body[name] = json!(value);
    }
}
