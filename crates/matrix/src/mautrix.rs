//! The mautrix bridge provider (ADR 0003): mautrix-go's bridgev2 provisioning API, v3, as a
//! [`BridgeProvider`].
//!
//! Each bridge's API is reached through the stack's proxy at
//! `<homeserver>/_pingcraft/bridges/<network>/_matrix/provision/v3/` (docs/deploy.md section 7). Every request
//! names the user (`user_id`) and carries an OpenID token as `Authorization: Bearer openid:<token>`: the bridge checks
//! it with the homeserver, so it learns who the user is and nothing more. The user's access token never goes to a
//! bridge. Tokens come from a [`TokenSource`]; on the desktop that is [`MatrixOpenId`].
//!
//! What the bridges send is hostile input: answers are size-limited, and a step, display or field type this
//! version does not know becomes [`LoginStep::Unsupported`] or [`FieldKind::Text`], never a parse error. Nothing
//! here logs a token, a phone number, a code or a password; the bridge's own error text is passed on as it came.
//!
//! Desktop only for now: `reqwest`'s futures are not `Send` in a browser, and providers' are. The web build gets
//! its own when it comes (milestone 4).

use std::sync::Arc;
use std::time::{Duration, Instant};

use matrix_sdk::Client;
use matrix_sdk::reqwest;
use pingcraft_core::bridge::{
    Answers, BoxFuture, BridgeError, BridgeProvider, BridgedNetwork, Display, FieldKind, InputField, LoginFlow, LoginStep, StepRef,
};
use pingcraft_core::{Account, AccountId, AccountState, Network};
use serde_json::{Map, Value};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

/// The networks PingCraft's stack routes a bridge for (deploy/Caddyfile).
pub const NETWORKS: &[&str] = &["telegram", "signal", "whatsapp"];

/// How long an ordinary request may take.
const TIMEOUT: Duration = Duration::from_secs(30);
/// How long waiting on a shown QR code or code may take before PingCraft asks again. The bridge keeps a login for
/// 30 minutes.
const WAIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The most PingCraft reads of one answer. A QR code and a few accounts are a few KiB.
const MAX_BODY: usize = 256 * 1024;
/// Most of what one list may hold (flows, fields, accounts); anything past it is dropped.
const MAX_ITEMS: usize = 64;
/// Most of one piece of text from a bridge (a name, an instruction, a QR code's content).
const MAX_TEXT: usize = 4096;

/// Where OpenID tokens for the signed-in user come from.
pub trait TokenSource: Send + Sync {
    /// A token that proves who the user is, valid for a while. Never logged.
    fn openid_token(&self) -> BoxFuture<'_, Result<Zeroizing<String>, BridgeError>>;
}

/// OpenID tokens from the user's homeserver (`/_matrix/client/v3/user/{userId}/openid/request_token`), kept until
/// shortly before they expire.
pub struct MatrixOpenId {
    client: Client,
    cached: Mutex<Option<(Zeroizing<String>, Instant)>>,
}

impl MatrixOpenId {
    pub fn new(client: Client) -> Self {
        Self { client, cached: Mutex::new(None) }
    }
}

impl TokenSource for MatrixOpenId {
    fn openid_token(&self) -> BoxFuture<'_, Result<Zeroizing<String>, BridgeError>> {
        Box::pin(async move {
            let mut cached = self.cached.lock().await;
            if let Some((token, until)) = cached.as_ref()
                && Instant::now() < *until
            {
                return Ok(token.clone());
            }
            let user = self.client.user_id().ok_or(BridgeError::NotAuthorized)?.to_owned();
            let request = matrix_sdk::ruma::api::client::account::request_openid_token::v3::Request::new(user);
            let response = self.client.send(request).await.map_err(|e| BridgeError::Unreachable(format!("no sign-in token from the server: {e}")))?;
            let token = Zeroizing::new(response.access_token);
            // A minute's margin, so a token is never sent just as it runs out.
            let lifetime = response.expires_in.saturating_sub(Duration::from_secs(60));
            *cached = Some((token.clone(), Instant::now() + lifetime));
            Ok(token)
        })
    }
}

/// The mautrix provider for one signed-in user on one server.
pub struct MautrixProvider {
    http: reqwest::Client,
    /// The homeserver's address, as the client uses it, without a trailing slash.
    base: String,
    user_id: String,
    tokens: Arc<dyn TokenSource>,
    networks: Vec<Network>,
}

impl MautrixProvider {
    /// A provider for the networks in [`NETWORKS`].
    pub fn new(homeserver: &str, user_id: &str, tokens: Arc<dyn TokenSource>) -> Self {
        Self::with_networks(homeserver, user_id, tokens, NETWORKS.iter().map(|n| Network::from_id(n)).collect())
    }

    pub fn with_networks(homeserver: &str, user_id: &str, tokens: Arc<dyn TokenSource>, networks: Vec<Network>) -> Self {
        Self { http: reqwest::Client::new(), base: homeserver.trim_end_matches('/').to_owned(), user_id: user_id.to_owned(), tokens, networks }
    }

    /// A provider for a signed-in client, with OpenID tokens from its homeserver.
    pub fn for_client(client: &Client) -> Option<Self> {
        let user = client.user_id()?.to_string();
        Some(Self::new(client.homeserver().as_str(), &user, Arc::new(MatrixOpenId::new(client.clone()))))
    }

    fn url(&self, network: &Network, path: &[&str], txn: Option<&str>) -> String {
        let mut url = format!("{}/_pingcraft/bridges/{}/_matrix/provision/v3", self.base, escape(network.id()));
        for segment in path {
            url.push('/');
            url.push_str(&escape(segment));
        }
        url.push_str("?user_id=");
        url.push_str(&escape(&self.user_id));
        if let Some(txn) = txn {
            url.push_str("&txn_id=");
            url.push_str(&escape(txn));
        }
        url
    }

    /// Sends one request and reads its JSON answer.
    async fn call(&self, method: reqwest::Method, url: String, body: Option<Zeroizing<Vec<u8>>>, timeout: Duration) -> Result<Value, BridgeError> {
        let token = self.tokens.openid_token().await?;
        let auth = Zeroizing::new(format!("Bearer openid:{}", token.as_str()));
        let mut request = self.http.request(method, url).header("authorization", auth.as_str()).timeout(timeout);
        if let Some(body) = body {
            request = request.header("content-type", "application/json").body(body.to_vec());
        }
        let mut response = request.send().await.map_err(|e| BridgeError::Unreachable(without_url(&e)))?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| BridgeError::Unreachable(without_url(&e)))? {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err(BridgeError::Unreachable("the bridge's answer is too large".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        let json: Option<Value> = serde_json::from_slice(&bytes).ok();
        if (200..300).contains(&status) {
            return json.ok_or_else(|| BridgeError::Unreachable("the bridge's answer is not JSON".into()));
        }
        Err(error_for(status, json.as_ref()))
    }

    async fn post(
        &self,
        network: &Network,
        path: &[&str],
        txn: Option<&str>,
        body: Option<Zeroizing<Vec<u8>>>,
        timeout: Duration,
    ) -> Result<Value, BridgeError> {
        self.call(reqwest::Method::POST, self.url(network, path, txn), body.or_else(|| Some(Zeroizing::new(b"{}".to_vec()))), timeout).await
    }

    async fn get(&self, network: &Network, path: &[&str]) -> Result<Value, BridgeError> {
        self.call(reqwest::Method::GET, self.url(network, path, None), None, TIMEOUT).await
    }
}

impl BridgeProvider for MautrixProvider {
    fn networks(&self) -> Vec<Network> {
        self.networks.clone()
    }

    fn network<'a>(&'a self, network: &'a Network) -> BoxFuture<'a, Result<BridgedNetwork, BridgeError>> {
        Box::pin(async move {
            let whoami = self.get(network, &["whoami"]).await?;
            Ok(bridged_network(network, &whoami))
        })
    }

    fn start_login<'a>(&'a self, network: &'a Network, flow: &'a str) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
        Box::pin(async move {
            let answer = self.post(network, &["login", "start", flow], None, None, TIMEOUT).await?;
            login_step(network, &answer)
        })
    }

    fn submit<'a>(&'a self, step: &'a StepRef, answers: Answers) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
        Box::pin(async move {
            let body: Map<String, Value> = answers.iter().map(|(k, v)| (k.to_owned(), Value::String(v.to_owned()))).collect();
            let body = Zeroizing::new(serde_json::to_vec(&body).map_err(|e| BridgeError::Unreachable(e.to_string()))?);
            let path = ["login", "step", step.login.as_str(), step.step.as_str(), "user_input"];
            let answer = self.post(&step.network, &path, Some(&step.attempt), Some(body), TIMEOUT).await?;
            login_step(&step.network, &answer)
        })
    }

    fn wait<'a>(&'a self, step: &'a StepRef) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
        Box::pin(async move {
            let path = ["login", "step", step.login.as_str(), step.step.as_str(), "display_and_wait"];
            let answer = self.post(&step.network, &path, Some(&step.attempt), None, WAIT_TIMEOUT).await?;
            login_step(&step.network, &answer)
        })
    }

    fn cancel<'a>(&'a self, step: &'a StepRef) -> BoxFuture<'a, Result<(), BridgeError>> {
        Box::pin(async move {
            match self.post(&step.network, &["login", "cancel", step.login.as_str()], None, None, TIMEOUT).await {
                Ok(_) | Err(BridgeError::LoginGone) => Ok(()),
                Err(e) => Err(e),
            }
        })
    }

    fn remove_account<'a>(&'a self, network: &'a Network, account: &'a AccountId) -> BoxFuture<'a, Result<(), BridgeError>> {
        Box::pin(async move {
            // "all" is the API's word for every login; an account with that ID is not one PingCraft made.
            if account.0 == "all" || account.0.is_empty() {
                return Err(BridgeError::Refused("not an account".into()));
            }
            match self.post(network, &["logout", account.0.as_str()], None, None, TIMEOUT).await {
                // Already gone is what was asked for.
                Ok(_) | Err(BridgeError::LoginGone) => Ok(()),
                Err(e) => Err(e),
            }
        })
    }
}

/// Escapes one URL path segment or query value: everything but RFC 3986's unreserved characters.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A request error without its URL, which holds the user ID.
fn without_url(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "it did not answer in time".into()
    } else if e.is_connect() {
        "no connection to the server".into()
    } else {
        "the request failed".into()
    }
}

/// What an error answer means. The proxy answers 404 without a body, and 502 or 503, when the network has no bridge.
fn error_for(status: u16, json: Option<&Value>) -> BridgeError {
    let code = json.and_then(|j| j.get("errcode")).and_then(Value::as_str).unwrap_or_default();
    let message = json.and_then(|j| j.get("error")).and_then(Value::as_str).map(|m| clip(m, 500)).unwrap_or_default();
    match (status, code) {
        (401, _) | (_, "M_UNKNOWN_TOKEN" | "M_MISSING_TOKEN") => BridgeError::NotAuthorized,
        (_, "FI.MAU.BRIDGE.LOGIN_CANCELLED" | "FI.MAU.BRIDGE.LOGIN_TIMED_OUT" | "FI.MAU.BRIDGE.LOGIN_ALREADY_FINISHED") => BridgeError::LoginGone,
        (404, "M_NOT_FOUND") if message.to_ascii_lowercase().contains("login") => BridgeError::LoginGone,
        (404, "") | (502 | 503, _) => BridgeError::NotAvailable,
        // A proxy in front gave up waiting (a shown QR code waits for minutes): the bridge is there.
        (504, _) => BridgeError::Unreachable("it did not answer in time".into()),
        (_, _) if !message.is_empty() => BridgeError::Refused(message),
        (status, _) => BridgeError::Unreachable(format!("the bridge answered {status}")),
    }
}

/// Text from a bridge, at most `max` characters.
fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).map(|s| clip(s, MAX_TEXT)).unwrap_or_default()
}

fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value.get(key).and_then(Value::as_array).into_iter().flatten().take(MAX_ITEMS)
}

/// A `whoami` answer as a network with its flows and the user's accounts on it.
fn bridged_network(network: &Network, whoami: &Value) -> BridgedNetwork {
    let flows = items(whoami, "login_flows")
        .filter(|f| !text(f, "id").is_empty())
        // mautrix's "manual" flow takes a session copied from elsewhere; its bridges call it "advanced, do not use".
        .filter(|f| text(f, "id") != "manual")
        .map(|f| LoginFlow { id: text(f, "id"), name: text(f, "name"), description: text(f, "description") })
        .collect();
    let accounts = items(whoami, "logins")
        .filter(|l| !text(l, "id").is_empty())
        .map(|l| {
            let profile = l.get("profile").unwrap_or(&Value::Null);
            let name = [text(l, "name"), text(profile, "name"), text(profile, "username"), text(profile, "phone"), text(l, "id")]
                .into_iter()
                .find(|n| !n.trim().is_empty())
                .unwrap_or_default();
            Account { id: AccountId(text(l, "id")), network: network.clone(), name, state: account_state(l.get("state").unwrap_or(&Value::Null)) }
        })
        .collect();
    BridgedNetwork { network: network.clone(), flows, accounts }
}

/// A login's bridge state (`state_event`) in PingCraft's words.
fn account_state(state: &Value) -> AccountState {
    let message = [text(state, "message"), text(state, "error")].into_iter().find(|m| !m.is_empty()).unwrap_or_default();
    match state.get("state_event").and_then(Value::as_str).unwrap_or_default() {
        "CONNECTED" | "BACKFILLING" => AccountState::Connected,
        "STARTING" | "CONNECTING" | "TRANSIENT_DISCONNECT" | "" => AccountState::Connecting,
        "LOGGED_OUT" | "BAD_CREDENTIALS" => AccountState::LoggedOut,
        other => AccountState::Error(if message.is_empty() { clip(other, 64) } else { message }),
    }
}

/// A login step from the bridge (`RespSubmitLogin`) as a [`LoginStep`].
fn login_step(network: &Network, answer: &Value) -> Result<LoginStep, BridgeError> {
    let kind = text(answer, "type");
    if kind == "complete" {
        let account = answer.pointer("/complete/user_login_id").and_then(Value::as_str).unwrap_or_default();
        return Ok(LoginStep::Done { network: network.clone(), account: AccountId(clip(account, MAX_TEXT)) });
    }
    let step = StepRef { network: network.clone(), login: text(answer, "login_id"), step: text(answer, "step_id"), attempt: text(answer, "txn_id") };
    if step.login.is_empty() || step.step.is_empty() {
        return Err(BridgeError::Unreachable("the bridge's login step has no ID".into()));
    }
    let instructions = text(answer, "instructions");
    match kind.as_str() {
        "display_and_wait" => {
            let params = answer.get("display_and_wait").unwrap_or(&Value::Null);
            let data = text(params, "data");
            let display = match text(params, "type").as_str() {
                "qr" if !data.is_empty() => Display::Qr(data),
                "code" if !data.is_empty() => Display::Code(data),
                "emoji" if !data.is_empty() => Display::Emoji(data),
                "nothing" => Display::Nothing,
                other => return Ok(LoginStep::Unsupported { step, kind: format!("display_and_wait/{}", clip(other, 32)), instructions }),
            };
            let can_cancel = params.get("can_cancel").and_then(Value::as_bool).unwrap_or(false);
            Ok(LoginStep::Show { step, instructions, display, can_cancel })
        }
        "user_input" => {
            let params = answer.get("user_input").unwrap_or(&Value::Null);
            let fields: Vec<InputField> = items(params, "fields").filter(|f| !text(f, "id").is_empty()).map(input_field).collect();
            if fields.is_empty() {
                return Ok(LoginStep::Unsupported { step, kind: "user_input/empty".into(), instructions });
            }
            // A field PingCraft cannot show (a choice from a list) makes the step one it cannot do.
            if let Some(select) = fields.iter().find(|f| f.kind == FieldKind::Text("select".into())) {
                return Ok(LoginStep::Unsupported { step, kind: format!("user_input/{}", select.id), instructions });
            }
            let can_cancel = params.get("can_cancel").and_then(Value::as_bool).unwrap_or(false);
            Ok(LoginStep::Ask { step, instructions, fields, can_cancel })
        }
        other => Ok(LoginStep::Unsupported { step, kind: clip(other, 32), instructions }),
    }
}

fn input_field(field: &Value) -> InputField {
    let kind = match text(field, "type").as_str() {
        "phone_number" => FieldKind::PhoneNumber,
        "2fa_code" | "captcha_code" => FieldKind::Code,
        "password" => FieldKind::Password,
        "username" => FieldKind::Username,
        "email" => FieldKind::Email,
        "token" => FieldKind::Token,
        "url" => FieldKind::Url,
        "domain" => FieldKind::Domain,
        other => FieldKind::Text(clip(other, 32)),
    };
    let default = text(field, "default_value");
    InputField {
        id: text(field, "id"),
        kind,
        name: text(field, "name"),
        description: text(field, "description"),
        default: (!default.is_empty()).then_some(default),
        optional: field.get("optional").and_then(Value::as_bool).unwrap_or(false),
    }
}

#[cfg(test)]
mod tests;
