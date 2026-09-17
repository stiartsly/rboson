use reqwest::{self, Method, StatusCode};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use url::Url;

use super::pow::{self, Solution};
use super::{
    base64url,
    errors::{
        ConflictError, ForbiddenError, InvalidRequestError, NotFoundError, PassphraseRequiredError,
        RateLimitError, RegistrationDisabledError, ServerError, ServiceBusyError,
        UnauthorizedError,
    },
    sign_nonce, Avatar, Device, Options, NodeStatus, Plan, Profile, ProfileUpdate,
    Subscription, UserPlan, UserRegistration,
};
use crate::{
    errors::{MalformedError, NetworkError, Result, StateError},
    signature,
    Id,
};

const API_PREFIX: &str = "api/v1/client";
const AUTH_NONCE_BYTES: usize = 32;

pub struct Client {
    client: reqwest::Client,
    base_url: Url,
    options: Options,
    device_id: Option<Id>,
    token: Mutex<Option<String>>,
    closed: AtomicBool,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("node_id", &self.options.node_id())
            .field("user_id", &self.options.user_id())
            .field("device_id", &self.device_id)
            .field("is_closed", &self.is_closed())
            .finish()
    }
}

impl Client {
    pub fn new(options: Options) -> Result<Self> {
        options.check_completeness()?;

        let mut base_url = options.director_url().clone();
        let path = base_url.path().trim_end_matches('/');
        let path = if path.ends_with(API_PREFIX) {
            format!("{path}/")
        } else {
            format!("{path}/{API_PREFIX}/")
        };
        base_url.set_path(&path);
        base_url.set_query(None);
        base_url.set_fragment(None);

        let mut b = reqwest::Client::builder().redirect(
            reqwest::redirect::Policy::none()
        );
        if options.is_insecure() {
            b = b.danger_accept_invalid_certs(true);
        }
        let client = b
            .build()
            .map_err(|e| NetworkError::new(e.to_string()))?;
        let device_id = options.device_private_key()
            .map(signature::KeyPair::from)
            .map(|kp| Id::from(kp.public_key()));

        Ok(Self {
            client,
            base_url,
            options,
            device_id,
            token: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn director_url(&self) -> &Url {
        &self.base_url
    }

    pub fn node_id(&self) -> Option<&Id> {
        self.options.node_id()
    }

    pub fn user_id(&self) -> Option<&Id> {
        self.options.user_id()
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

    pub async fn fetch_node_id(&self) -> Result<Id> {
        self.check_open()?;
        let body = self.http_get("id", false).await?;
        let node_id = body
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| MalformedError::new("missing 'id'"))?;

        let id = node_id.parse::<Id>().map_err(|e| {
            MalformedError::new(format!("error parsing node id {e}"))
        })?;
        Ok(id)
    }

    pub async fn fetch_node_status(&self) -> Result<NodeStatus> {
        self.check_open()?;
        self.http_get_json("node", false).await
    }

    pub async fn register_user(&self) -> Result<()> {
        let Some(registration) = self.options.registration() else {
            return Err(StateError::new("No registration configured").into());
        };
        self.register_user_with(registration).await
    }

    pub async fn register_user_with(&self, registration: &UserRegistration) -> Result<()> {
        self.check_open()?;

        let user_key = self
            .options
            .user_private_key()
            .map(signature::KeyPair::from)
            .ok_or_else(|| StateError::new("Registering a user needs the user key"))?;

        let initial_device_key = if registration.has_initial_device() {
            let device_key = self
                .options
                .device_private_key()
                .map(signature::KeyPair::from)
                .ok_or_else(|| {
                    StateError::new("Registering an initial device needs the device key")
                })?;
            Some(device_key)
        } else {
            None
        };

        let node_id = match self.options.node_id() {
            Some(node_id) => node_id.clone(),
            _ => self.fetch_node_id().await?,
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
            registration,
            node_id,
            &user_key,
            initial_device_key.as_ref(),
            &challenge,
            solution,
        )
        .await
    }

    pub async fn register_device(
        &self,
        name: &str,
        app: &str,
        passphrase: Option<&str>,
    ) -> Result<()> {
        let key = self
            .options
            .device_private_key()
            .map(signature::KeyPair::from)
            .ok_or_else(|| {
                StateError::new("No device key configured; pass a device key to register")
            })?;

        self.register_device_with_key(&key, name, app, passphrase)
            .await
    }

    pub async fn register_device_with_key(
        &self,
        key: &signature::KeyPair,
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
            "deviceSig": sign_nonce(key.private_key(), &nonce).map_err(|e|
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
        user_key: &signature::KeyPair,
        initial_device_key: Option<&signature::KeyPair>,
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
        if self.options.user_id().is_none() {
            Err(StateError::new("This call needs a user identity"))
        } else {
            Ok(())
        }
    }

    async fn http_get(&self, path: &str, authenticated: bool) -> Result<Value> {
        self.http_call(Method::GET, path, None, None, authenticated)
            .await
    }

    async fn http_get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        authenticated: bool,
    ) -> Result<T> {
        let response = self
            .raw(Method::GET, path, None, None, authenticated)
            .await?;
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
        let mut request = self.client.request(method, self.url(path)?);
        if let Some(token) = token.as_ref() {
            request = request.bearer_auth(token);
        }
        if let Some(json) = json {
            request = request.json(&json);
        }
        if let Some((body, content_type)) = binary {
            request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body);
        }
        let response = request
            .send()
            .await
            .map_err(|e| NetworkError::new(e.to_string()))?;
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
            .options
            .user_id()
            .copied()
            .ok_or_else(|| StateError::new("This call needs a user identity"))?;
        let nonce = crate::random_array::<AUTH_NONCE_BYTES>();
        let mut body = json!({"userId": user_id, "nonce": base64url(&nonce)});
        if let Some(key) = self.options.user_private_key() {
            body["userSig"] =
                json!(sign_nonce(key, &nonce).map_err(|e| MalformedError::new(e.to_string()))?);
        } else if let Some(key) = self.options.device_private_key() {
            body["deviceId"] = json!(self.device_id);
            body["deviceSig"] =
                json!(sign_nonce(key, &nonce).map_err(|e| MalformedError::new(e.to_string()))?);
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
