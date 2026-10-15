//! Official Sign in with ChatGPT OSS flow. No Codex credentials/private APIs.
//! Secrets never cross Tauri IPC; login is explicit and recording is independent.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use fs2::FileExt;
use futures_util::StreamExt;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ISSUER: &str = "https://auth.openai.com";
const DISCOVERY: &str = "https://auth.openai.com/.well-known/openid-configuration";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const AGENT_NAME: &str = "Meetily";
static STORE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LOGIN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_value() -> String {
    let mut bytes = [0u8; 48];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("meetily-live-minutes/0.1")
        .build()
        .map_err(|_| "Unable to create SIWC HTTP client".into())
}

// Deliberately no Debug on secret-bearing types.
#[derive(Clone, Serialize, Deserialize)]
struct Tokens {
    access: Option<String>,
    refresh: Option<String>,
    id: Option<String>,
    scopes: Vec<String>,
    expires_at: u64,
    earliest_refresh_at: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Registration {
    client_id: String,
    subject: Option<String>,
    email: Option<String>,
    tokens: Option<Tokens>,
}
#[derive(Serialize, Deserialize)]
struct Database {
    version: u32,
    active: Option<String>,
    registrations: Vec<Registration>,
}
impl Default for Database {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            registrations: Vec::new(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Account {
    pub client_id: String,
    pub label: String,
    pub connected: bool,
    pub plan_enabled: bool,
}
#[derive(Debug, Serialize)]
pub struct Status {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}
fn plan_enabled(tokens: &Tokens) -> bool {
    ["chatgpt.tokens.use.direct", "resource.invoke"]
        .iter()
        .all(|s| tokens.scopes.iter().any(|v| v == s))
}
fn public_status(db: &Database) -> Status {
    Status {
        active: db.active.clone(),
        accounts: db
            .registrations
            .iter()
            .map(|r| Account {
                client_id: r.client_id.clone(),
                label: format!(
                    "{} · {}",
                    r.email.as_deref().unwrap_or(if r.subject.is_none() {
                        "未完了の登録"
                    } else {
                        "ChatGPT account"
                    }),
                    r.client_id
                        .chars()
                        .rev()
                        .take(8)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                ),
                connected: r.tokens.is_some(),
                plan_enabled: r.tokens.as_ref().is_some_and(plan_enabled),
            })
            .collect(),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing auth directory")?)
        .map_err(|_| "Unable to create protected credential file")?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|_| "Unable to write credentials")?;
    temp.persist(path)
        .map_err(|_| "Unable to atomically replace credentials")?;
    #[cfg(unix)]
    File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(|_| "Unable to sync credential directory")?;
    Ok(())
}

#[cfg(windows)]
fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "Credentials too large")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI user scope: disk files contain ciphertext, never plaintext tokens.
    let success = unsafe {
        if decrypt {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 || output.pbData.is_null() {
        return Err("Windows credential protection failed; no plaintext fallback".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}
#[cfg(not(windows))]
fn protect(bytes: &[u8], _decrypt: bool) -> Result<Vec<u8>, String> {
    Ok(bytes.to_vec())
}

struct Store {
    dir: PathBuf,
    lock: File,
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}
impl Store {
    fn open(dir: &Path) -> Result<Self, String> {
        if std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Auth directory must not be a symlink".into());
        }
        std::fs::create_dir_all(dir).map_err(|_| "Unable to create auth directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Unable to protect auth directory")?;
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(dir.join("session.lock"))
                .map_err(|_| "Unable to open auth lock")?;
            FileExt::try_lock_exclusive(&lock)
                .map_err(|_| "CHATGPT_TEMPORARY: Credentials are in use by another process")?;
            Ok(Self {
                dir: dir.into(),
                lock,
            })
        }
        #[cfg(not(unix))]
        {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(dir.join("session.lock"))
                .map_err(|_| "Unable to open auth lock")?;
            FileExt::try_lock_exclusive(&lock)
                .map_err(|_| "CHATGPT_TEMPORARY: Credentials are in use by another process")?;
            Ok(Self {
                dir: dir.into(),
                lock,
            })
        }
    }
    fn path(&self) -> PathBuf {
        self.dir.join(if cfg!(windows) {
            "credentials.dpapi"
        } else {
            "credentials.json"
        })
    }
    fn load(&self) -> Result<Database, String> {
        if let Ok(metadata) = std::fs::symlink_metadata(self.path()) {
            if metadata.file_type().is_symlink() || metadata.len() > 4_000_000 {
                return Err("Unsafe credential store path or size".into());
            }
        }
        let bytes = match std::fs::read(self.path()) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Database::default()),
            Err(_) => return Err("Cannot read SIWC credential store".into()),
        };
        let db: Database = serde_json::from_slice(&protect(&bytes, true)?)
            .map_err(|_| "Invalid SIWC credential store; refusing to overwrite it")?;
        if db.version != 1 {
            return Err("Unsupported credential version".into());
        }
        Ok(db)
    }
    fn save(&self, db: &Database) -> Result<(), String> {
        let bytes = serde_json::to_vec(db).map_err(|_| "Unable to serialize credentials")?;
        atomic_write(&self.path(), &protect(&bytes, false)?)
    }
    fn host_id(&self) -> Result<String, String> {
        let path = self.dir.join("host-id");
        match std::fs::read_to_string(&path) {
            Ok(v) if v.starts_with("urn:uuid:") && uuid::Uuid::parse_str(&v[9..]).is_ok() => Ok(v),
            Ok(_) => Err("Invalid existing host identity; refusing to replace it".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !self.load()?.registrations.is_empty() {
                    return Err("Existing registrations have lost their host identity; refusing silent host rotation".into());
                }
                let id = format!("urn:uuid:{}", uuid::Uuid::new_v4());
                atomic_write(&path, id.as_bytes())?;
                Ok(id)
            }
            Err(_) => Err("Unable to read host identity".into()),
        }
    }
}
fn directory<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("official-siwc"))
        .map_err(|_| "App config directory unavailable".into())
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    revocation_endpoint: String,
    jwks_uri: String,
}
async fn discovery(client: &reqwest::Client) -> Result<Discovery, String> {
    let d: Discovery = client
        .get(DISCOVERY)
        .send()
        .await
        .map_err(|_| "OpenAI discovery unavailable")?
        .error_for_status()
        .map_err(|_| "OpenAI discovery rejected")?
        .json()
        .await
        .map_err(|_| "Invalid OpenAI discovery")?;
    if d.issuer != ISSUER {
        return Err("Unexpected OIDC issuer".into());
    }
    for endpoint in [
        &d.authorization_endpoint,
        &d.token_endpoint,
        &d.revocation_endpoint,
        &d.jwks_uri,
    ] {
        let u = url::Url::parse(endpoint).map_err(|_| "Invalid discovery endpoint")?;
        if u.scheme() != "https"
            || u.host_str() != Some("auth.openai.com")
            || u.port().is_some()
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err("Untrusted OpenAI discovery endpoint".into());
        }
    }
    Ok(d)
}
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
    iat: u64,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    email: Option<String>,
}
fn verify_identity(
    token: &str,
    client_id: &str,
    nonce: Option<&str>,
    jwks: &JwkSet,
) -> Result<Claims, String> {
    let header = decode_header(token).map_err(|_| "Invalid ID token header")?;
    if header.alg != Algorithm::RS256 {
        return Err("Unexpected ID token signing algorithm".into());
    }
    let key = jwks
        .find(header.kid.as_deref().ok_or("ID token has no kid")?)
        .ok_or("Unknown ID token signing key")?;
    let key = DecodingKey::from_jwk(key).map_err(|_| "Invalid ID signing key")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.set_required_spec_claims(&["sub", "iss", "aud", "exp", "iat"]);
    validation.leeway = 5;
    validation.validate_nbf = true;
    let claims = decode::<Claims>(token, &key, &validation)
        .map_err(|_| "ID token signature/issuer/audience/expiry verification failed")?
        .claims;
    if claims.sub.is_empty()
        || claims.iat > now() + 5
        || nonce.is_some_and(|n| claims.nonce.as_deref() != Some(n))
    {
        return Err("ID token subject, issue time or nonce mismatch".into());
    }
    Ok(claims)
}
async fn identity(
    client: &reqwest::Client,
    d: &Discovery,
    token: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> Result<Claims, String> {
    // Fetch current published keys for each login/refresh: unknown kid gets a fresh JWKS.
    let keys: JwkSet = client
        .get(&d.jwks_uri)
        .send()
        .await
        .map_err(|_| "JWKS unavailable")?
        .error_for_status()
        .map_err(|_| "JWKS rejected")?
        .json()
        .await
        .map_err(|_| "Invalid JWKS")?;
    verify_identity(token, client_id, nonce, &keys)
}
#[derive(Deserialize)]
struct Grant {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    scope: String,
    #[serde(default)]
    earliest_refresh_at: Option<u64>,
}
fn grant_tokens(g: Grant, old: Option<&Tokens>) -> Result<Tokens, String> {
    if g.access_token.is_some()
        && !g
            .token_type
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("Bearer"))
    {
        return Err("Unexpected OAuth token type".into());
    }
    if g.access_token.as_deref() == Some("")
        || g.refresh_token.as_deref() == Some("")
        || g.id_token.as_deref() == Some("")
    {
        return Err("OAuth returned an empty credential".into());
    }
    if old.is_some()
        && (g.access_token.is_none() || g.refresh_token.is_none() || g.expires_in.unwrap_or(0) == 0)
    {
        return Err("Incomplete rotated token response; credentials not replaced".into());
    }
    Ok(Tokens {
        access: g.access_token,
        refresh: g
            .refresh_token
            .or_else(|| old.and_then(|t| t.refresh.clone())),
        id: g.id_token.or_else(|| old.and_then(|t| t.id.clone())),
        scopes: g.scope.split_whitespace().map(str::to_owned).collect(),
        expires_at: now().saturating_add(g.expires_in.unwrap_or(0)),
        earliest_refresh_at: g.earliest_refresh_at,
    })
}
async fn exchange(
    client: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<Grant, String> {
    let response = client
        .post(endpoint)
        .form(form)
        .send()
        .await
        .map_err(|_| "CHATGPT_TEMPORARY: OAuth endpoint unavailable")?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let code = body["error"]
            .as_str()
            .or_else(|| body["error"]["code"].as_str())
            .unwrap_or("unknown_error");
        let terminal = [
            "invalid_grant",
            "invalid_refresh_token",
            "token_expired",
            "refresh_token_expired",
            "refresh_token_invalidated",
            "refresh_token_reused",
        ]
        .contains(&code);
        return Err(format!(
            "{}: OAuth HTTP {} ({})",
            if terminal {
                "CHATGPT_SIGN_IN_REQUIRED"
            } else if code == "invalid_client" {
                "CHATGPT_AUTH_CONFIG"
            } else {
                "CHATGPT_TEMPORARY"
            },
            status.as_u16(),
            code
        ));
    }
    serde_json::from_value(body).map_err(|_| "Invalid OAuth token response".into())
}

struct Attempt {
    state: String,
    nonce: String,
    verifier: String,
    redirect: String,
}
impl Attempt {
    fn new(port: u16) -> Self {
        Self {
            state: random_value(),
            nonce: random_value(),
            verifier: random_value(),
            redirect: format!("http://127.0.0.1:{port}/auth/callback"),
        }
    }
    fn authorization(
        &self,
        d: &Discovery,
        host: &str,
        selected: Option<&Registration>,
        consent: bool,
    ) -> Result<url::Url, String> {
        let mut u = url::Url::parse(&d.authorization_endpoint)
            .map_err(|_| "Invalid authorization endpoint")?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        let mut q = u.query_pairs_mut();
        q.extend_pairs([
            (
                "client_id",
                selected.map_or("dynamic_agent_client", |r| r.client_id.as_str()),
            ),
            ("ext_agent_host_id", host),
            ("redirect_uri", &self.redirect),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("resource", RESOURCE),
            ("state", &self.state),
            ("nonce", &self.nonce),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
        ]);
        if let Some(r) = selected {
            if let Some(id) = r.tokens.as_ref().and_then(|t| t.id.as_deref()) {
                q.append_pair("id_token_hint", id);
            }
            if let Some(email) = &r.email {
                q.append_pair("login_hint", email);
            }
        } else {
            q.append_pair("agent_name_hint", AGENT_NAME);
        }
        if consent {
            q.append_pair("prompt", "consent");
        }
        drop(q);
        Ok(u)
    }
    fn callback(
        &self,
        target: &str,
        selected: Option<&Registration>,
    ) -> Result<(String, String), String> {
        let u = url::Url::parse(&format!("http://127.0.0.1{target}"))
            .map_err(|_| "Invalid callback URL")?;
        if u.path() != "/auth/callback" {
            return Err("Wrong callback path".into());
        }
        let mut values = std::collections::HashMap::new();
        for (k, v) in u.query_pairs() {
            if values.insert(k.into_owned(), v.into_owned()).is_some() {
                return Err("Duplicate callback parameter".into());
            }
        }
        if values.get("state") != Some(&self.state) {
            return Err("Callback state mismatch".into());
        }
        if values.contains_key("error") {
            return Err("ChatGPT authorization was declined or cancelled".into());
        }
        let id = match selected {
            Some(r) => {
                if values.get("client_id").is_some_and(|id| id != &r.client_id) {
                    return Err("Returning registration mismatch".into());
                }
                r.client_id.clone()
            }
            None => values
                .get("client_id")
                .filter(|s| !s.is_empty() && s.as_str() != "dynamic_agent_client")
                .cloned()
                .ok_or("Registration did not issue a client ID")?,
        };
        let code = values
            .get("code")
            .filter(|s| !s.is_empty())
            .cloned()
            .ok_or("Missing authorization code")?;
        Ok((code, id))
    }
}

async fn read_callback_request(socket: &mut tokio::net::TcpStream) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let count = socket.read(&mut buf).await.ok()?;
            if count == 0 {
                return None;
            }
            request.extend_from_slice(&buf[..count]);
            if request.len() > 8192 {
                return None;
            }
            if request.windows(4).any(|v| v == b"\r\n\r\n") {
                return String::from_utf8(request).ok();
            }
        }
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub async fn chatgpt_status<R: Runtime>(app: AppHandle<R>) -> Result<Status, String> {
    status_in_directory(&directory(&app)?).await
}
async fn status_in_directory(dir: &Path) -> Result<Status, String> {
    let _guard = STORE_LOCK.lock().await;
    Ok(public_status(&Store::open(dir)?.load()?))
}
pub(crate) async fn summary_context<R: Runtime>(app: &AppHandle<R>) -> Result<(PathBuf, String), String> {
    let dir = directory(app)?;
    let status = status_in_directory(&dir).await?;
    let account = status.active.filter(|id| status.accounts.iter().any(|a|
        &a.client_id == id && a.connected && a.plan_enabled))
        .ok_or("Connect ChatGPT and grant plan access in Settings.")?;
    Ok((dir, account))
}
async fn require_active_account(dir: &Path, account: &str) -> Result<(), String> {
    let status = status_in_directory(dir).await?;
    if status.active.as_deref() == Some(account) && status.accounts.iter().any(|a|
        a.client_id == account && a.connected && a.plan_enabled) { Ok(()) }
    else { Err("CHATGPT_ACCOUNT_CHANGED: Request or response discarded".into()) }
}
#[tauri::command]
pub async fn chatgpt_select_account<R: Runtime>(
    app: AppHandle<R>,
    client_id: String,
) -> Result<Status, String> {
    let _guard = STORE_LOCK.lock().await;
    let store = Store::open(&directory(&app)?)?;
    let mut db = store.load()?;
    if !db.registrations.iter().any(|r| r.client_id == client_id) {
        return Err("Unknown registration".into());
    }
    db.active = Some(client_id);
    store.save(&db)?;
    Ok(public_status(&db))
}
#[tauri::command]
pub async fn chatgpt_sign_in<R: Runtime>(
    app: AppHandle<R>,
    client_id: Option<String>,
    consent: bool,
) -> Result<Status, String> {
    let _login = LOGIN_LOCK
        .try_lock()
        .map_err(|_| "ChatGPT sign-in already pending")?;
    let dir = directory(&app)?;
    let (host, selected) = {
        let _guard = STORE_LOCK.lock().await;
        let store = Store::open(&dir)?;
        let db = store.load()?;
        let selected = match client_id {
            Some(id) => Some(
                db.registrations
                    .into_iter()
                    .find(|r| r.client_id == id)
                    .ok_or("Unknown saved registration")?,
            ),
            None => None,
        };
        (store.host_id()?, selected)
    };
    let client = http_client()?;
    let d = discovery(&client).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Unable to bind OAuth loopback listener")?;
    let attempt = Attempt::new(
        listener
            .local_addr()
            .map_err(|_| "No loopback address")?
            .port(),
    );
    let url = attempt.authorization(&d, &host, selected.as_ref(), consent)?;
    // Native-only; no auth URL (potential ID-token hint) goes to JS or logs.
    let opened = tokio::task::spawn_blocking(move || {
        for mut command in open::commands(url.as_str()) {
            if let Ok(mut child) = command
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                // Never wait for a browser's lifetime before accepting its callback.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return true;
            }
        }
        false
    })
    .await
    .map_err(|_| "Browser launch failed")?;
    if !opened {
        return Err("Unable to open system browser; sign-in was not started".into());
    }
    let (code, issued_id) = tokio::time::timeout(Duration::from_secs(300), async {
        loop {
            let (mut socket, _) = listener.accept().await.map_err(|_| "Callback listener failed")?;
            let Some(request) = read_callback_request(&mut socket).await else { continue };
            let mut parts = request.lines().next().unwrap_or("").split_whitespace();
            let result = if parts.next() == Some("GET") { attempt.callback(parts.next().unwrap_or(""), selected.as_ref()) } else { Err("Expected GET callback".into()) };
            let text = if result.is_ok() { "Authorization received. Return to Meetily for validation." } else { "Invalid or cancelled authorization. Return to Meetily." };
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", text.len(), text);
            let _ = socket.write_all(response.as_bytes()).await;
            match result {
                Ok(v) => return Ok::<_,String>(v),
                Err(e) if ["Callback state mismatch", "Wrong callback path", "Duplicate callback parameter", "Expected GET callback", "Invalid callback URL"].contains(&e.as_str()) => continue,
                Err(e) => return Err(e),
            }
        }
    }).await.map_err(|_| "ChatGPT sign-in timed out; no active account was changed")??;
    // Retain an issued, incomplete registration on exchange failure. It is NOT an
    // authenticated identity and never replaces the active account.
    {
        let _guard = STORE_LOCK.lock().await;
        let store = Store::open(&dir)?;
        let mut db = store.load()?;
        if !db.registrations.iter().any(|r| r.client_id == issued_id) {
            db.registrations.push(Registration {
                client_id: issued_id.clone(),
                subject: None,
                email: None,
                tokens: None,
            });
            store.save(&db)?;
        }
    }
    let grant = exchange(
        &client,
        &d.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &issued_id),
            ("code", &code),
            ("code_verifier", &attempt.verifier),
            ("redirect_uri", &attempt.redirect),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    let claims = identity(
        &client,
        &d,
        grant
            .id_token
            .as_deref()
            .ok_or("OAuth response missing ID token")?,
        &issued_id,
        Some(&attempt.nonce),
    )
    .await?;
    if selected
        .as_ref()
        .and_then(|r| r.subject.as_deref())
        .is_some_and(|s| s != claims.sub)
    {
        return Err("Verified returning account does not match selected registration".into());
    }
    let registration = Registration {
        client_id: issued_id.clone(),
        subject: Some(claims.sub),
        email: claims.email,
        tokens: Some(grant_tokens(grant, None)?),
    };
    let _guard = STORE_LOCK.lock().await;
    let store = Store::open(&dir)?;
    let mut db = store.load()?;
    if let Some(r) = db
        .registrations
        .iter_mut()
        .find(|r| r.client_id == issued_id)
    {
        if r.subject.is_some() && r.subject != registration.subject {
            return Err("Saved registration identity mismatch".into());
        }
        *r = registration;
    } else {
        db.registrations.push(registration);
    }
    db.active = Some(issued_id);
    store.save(&db)?;
    Ok(public_status(&db))
}

async fn access(dir: &Path, expected_account: &str) -> Result<String, String> {
    let _guard = STORE_LOCK.lock().await;
    let store = Store::open(dir)?;
    let mut db = store.load()?;
    if db.active.as_deref() != Some(expected_account) {
        return Err("CHATGPT_ACCOUNT_CHANGED: Reselect the account and model".into());
    }
    let r = db
        .registrations
        .iter_mut()
        .find(|r| r.client_id == expected_account)
        .ok_or("CHATGPT_SIGN_IN_REQUIRED: No selected registration")?;
    let old = r
        .tokens
        .clone()
        .ok_or("CHATGPT_SIGN_IN_REQUIRED: Sign in first")?;
    if !plan_enabled(&old) {
        return Err(
            "CHATGPT_PERMISSION_REQUIRED: Consent to ChatGPT plan usage is not granted".into(),
        );
    }
    if old.expires_at > now() + 60
        || (old.expires_at > now() && old.earliest_refresh_at.is_some_and(|t| t > now()))
    {
        return old
            .access
            .ok_or("CHATGPT_SIGN_IN_REQUIRED: No access token".into());
    }
    if old.earliest_refresh_at.is_some_and(|t| t > now()) {
        return Err("CHATGPT_TEMPORARY: Refresh is not permitted yet".into());
    }
    let client = http_client()?;
    let d = discovery(&client).await?;
    let grant = match exchange(
        &client,
        &d.token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &r.client_id),
            (
                "refresh_token",
                old.refresh
                    .as_deref()
                    .ok_or("CHATGPT_SIGN_IN_REQUIRED: No renewable session")?,
            ),
            ("resource", RESOURCE),
        ],
    )
    .await
    {
        Ok(g) => g,
        Err(e) => {
            if e.starts_with("CHATGPT_SIGN_IN_REQUIRED:") {
                r.tokens = None;
                store.save(&db)?;
            }
            return Err(e);
        }
    };
    if let Some(id) = &grant.id_token {
        let claims = identity(&client, &d, id, &r.client_id, None).await?;
        if Some(&claims.sub) != r.subject.as_ref() {
            return Err("Refreshed identity mismatch".into());
        }
    }
    let tokens = grant_tokens(grant, Some(&old))?;
    let permitted = plan_enabled(&tokens);
    let token = tokens.access.clone();
    r.tokens = Some(tokens);
    store.save(&db)?;
    if !permitted {
        return Err(
            "CHATGPT_PERMISSION_REQUIRED: Refresh no longer grants ChatGPT plan usage".into(),
        );
    }
    token.ok_or("CHATGPT_SIGN_IN_REQUIRED: Refreshed access token missing".into())
}
#[tauri::command]
pub async fn chatgpt_sign_out<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let _guard = STORE_LOCK.lock().await;
    let store = Store::open(&directory(&app)?)?;
    let mut db = store.load()?;
    let r = db
        .registrations
        .iter_mut()
        .find(|r| Some(&r.client_id) == db.active.as_ref())
        .ok_or("No selected account")?;
    let mut revoked = r.tokens.as_ref().and_then(|t| t.refresh.as_ref()).is_none();
    if let Some(refresh) = r.tokens.as_ref().and_then(|t| t.refresh.as_deref()) {
        if let Ok(client) = http_client() {
            if let Ok(d) = discovery(&client).await {
                for attempt in 0..2 {
                    if attempt > 0 {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    if client
                        .post(&d.revocation_endpoint)
                        .form(&[
                            ("token", refresh),
                            ("token_type_hint", "refresh_token"),
                            ("client_id", r.client_id.as_str()),
                        ])
                        .send()
                        .await
                        .is_ok_and(|v| v.status().as_u16() == 200)
                    {
                        revoked = true;
                        break;
                    }
                }
            }
        }
    }
    r.tokens = None;
    store.save(&db)?;
    Ok(if revoked { "Signed out" } else { "Signed out locally; remote revocation was not confirmed. Disconnect this app in ChatGPT Settings." }.into())
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub slug: String,
    pub display_name: String,
}
fn models(body: Value) -> Result<Vec<Model>, String> {
    body["models"]
        .as_array()
        .ok_or("Account-specific model catalog missing")?
        .iter()
        .filter(|m| m["visibility"] == "list")
        .map(|m| {
            serde_json::from_value(m.clone()).map_err(|_| "Invalid model catalog entry".into())
        })
        .collect()
}
fn api_error(status: u16, body: &Value, request_id: &str) -> String {
    let error = body.get("error").unwrap_or(body);
    let code = error["code"]
        .as_str()
        .unwrap_or("unstructured_admission_error");
    let prefix = match code {
        "subscription_sharing_usage_limit_exceeded" => "CHATGPT_USAGE_LIMIT",
        "subscription_sharing_user_not_eligible"
        | "subscription_sharing_route_not_supported"
        | "chatpass_v2_scope_not_authorized"
        | "chatpass_v2_invalid_authorization_context" => "CHATGPT_RESTRICTED",
        "subscription_sharing_unsupported_capability" => "CHATGPT_INVALID_REQUEST",
        "subscription_sharing_invalid_user" => "CHATGPT_AUTH_REJECTED",
        "subscription_sharing_usage_unavailable"
        | "subscription_sharing_user_unavailable"
        | "server_error"
        | "rate_limit_exceeded" => "CHATGPT_TEMPORARY",
        _ if status == 401 => "CHATGPT_AUTH_REJECTED",
        _ if status == 403 => "CHATGPT_RESTRICTED",
        _ if status == 429 => "CHATGPT_USAGE_LIMIT",
        _ if status >= 500 => "CHATGPT_TEMPORARY",
        _ => "CHATGPT_INVALID_REQUEST",
    };
    let detail = body["detail"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    format!("{prefix}: HTTP {status}; code={code}; param={}; request_id={request_id}; body_shape={}; detail={detail}", error["param"].as_str().unwrap_or("none"), if body.get("detail").is_some() { "detail" } else { "error" })
}
#[tauri::command]
pub async fn chatgpt_list_models<R: Runtime>(
    app: AppHandle<R>,
    client_id: String,
) -> Result<Vec<Model>, String> {
    let dir = directory(&app)?;
    let token = access(&dir, &client_id).await?;
    let response = http_client()?
        .get(format!("{RESOURCE}/models"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "CHATGPT_TEMPORARY: Model lookup failed")?;
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = response.json().await.unwrap_or(Value::Null);
    if status != 200 {
        return Err(api_error(status, &body, &request_id));
    }
    let status = chatgpt_status(app).await?;
    if status.active.as_deref() != Some(&client_id) {
        return Err("CHATGPT_ACCOUNT_CHANGED: Model lookup discarded".into());
    }
    models(body)
}
fn responses_body(model: &str, instructions: &str, input: &str) -> Value {
    json!({"model":model, "instructions":instructions, "input":[{"role":"user","content":input}], "store":false,"stream":true})
}
// Frame buffering operates on bytes, preserving split multibyte Japanese characters.
#[derive(Default)]
struct Sse {
    pending: Vec<u8>,
    text: String,
    completed: bool,
    http_status: u16,
    request_id: String,
}
impl Sse {
    fn push(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > 4_000_000 || self.text.len() > 1_000_000 {
            return Err("Response exceeded bounded output size".into());
        }
        loop {
            let delimiter = [
                self.pending
                    .windows(2)
                    .position(|v| v == b"\n\n")
                    .map(|p| (p, 2)),
                self.pending
                    .windows(4)
                    .position(|v| v == b"\r\n\r\n")
                    .map(|p| (p, 4)),
            ]
            .into_iter()
            .flatten()
            .min_by_key(|(position, _)| *position);
            let Some((index, length)) = delimiter else {
                break;
            };
            let frame: Vec<u8> = self.pending.drain(..index + length).collect();
            let frame = std::str::from_utf8(&frame).map_err(|_| "Invalid SSE UTF-8")?;
            let data = frame
                .lines()
                .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let event: Value =
                serde_json::from_str(&data).map_err(|_| "Invalid Responses event")?;
            match event["type"].as_str().unwrap_or("") {
                "response.output_text.delta" => self
                    .text
                    .push_str(event["delta"].as_str().ok_or("Missing output delta")?),
                "response.refusal.delta" => {
                    return Err(
                        "CHATGPT_REFUSAL: Model declined the request; draft discarded".into(),
                    )
                }
                "response.completed" => {
                    if event["response"]["status"]
                        .as_str()
                        .is_some_and(|s| s != "completed")
                    {
                        return Err("Response not completed".into());
                    }
                    if let Some(output) = event["response"]["output"].as_array() {
                        let mut text = String::new();
                        for message in output {
                            if let Some(content) = message["content"].as_array() {
                                for part in content {
                                    if part["type"] == "refusal" {
                                        return Err(
                                            "CHATGPT_REFUSAL: Model declined the request".into()
                                        );
                                    }
                                    if part["type"] == "output_text" {
                                        text.push_str(
                                            part["text"]
                                                .as_str()
                                                .ok_or("Missing final output text")?,
                                        );
                                    }
                                }
                            }
                        }
                        if !text.is_empty() {
                            self.text = text;
                        }
                    }
                    self.completed = true;
                }
                "response.incomplete" => {
                    return Err("CHATGPT_INCOMPLETE: Response incomplete; draft discarded".into())
                }
                "response.failed" | "error" => {
                    let body = if event.get("response").is_some() {
                        &event["response"]
                    } else {
                        &event
                    };
                    return Err(api_error(self.http_status, body, &self.request_id));
                }
                _ => {}
            }
            if self.text.len() > 1_000_000 {
                return Err("Response exceeded bounded output size".into());
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<String, String> {
        if !self.completed {
            return Err(
                "CHATGPT_TEMPORARY: Stream ended without response.completed; draft discarded"
                    .into(),
            );
        }
        if self.text.trim().is_empty() {
            return Err("Completed response contained no text".into());
        }
        Ok(self.text)
    }
}
pub async fn generate<R: Runtime>(
    app: &AppHandle<R>,
    account: &str,
    model: &str,
    instructions: &str,
    input: &str,
) -> Result<String, String> {
    generate_in_directory(&directory(app)?, account, model, instructions, input).await
}
pub(crate) async fn generate_in_directory(
    dir: &Path, account: &str, model: &str, instructions: &str, input: &str,
) -> Result<String, String> {
    require_active_account(dir, account).await?;
    let token = access(dir, account).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Unable to create inference client")?;
    let response = client
        .post(format!("{RESOURCE}/responses"))
        .bearer_auth(token)
        .json(&responses_body(model, instructions, input))
        .send()
        .await
        .map_err(|_| "CHATGPT_TEMPORARY: Inference unavailable")?;
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if status != 200 {
        let body = response.json().await.unwrap_or(Value::Null);
        return Err(api_error(status, &body, &request_id));
    }
    let mut stream = response.bytes_stream();
    let mut sse = Sse {
        http_status: status,
        request_id,
        ..Sse::default()
    };
    while let Some(bytes) = stream.next().await {
        sse.push(&bytes.map_err(|_| "CHATGPT_TEMPORARY: Responses stream interrupted")?)?;
        if sse.completed {
            break;
        }
    }
    let text = sse.finish()?;
    require_active_account(dir, account).await?;
    Ok(text)
}

#[tauri::command]
pub async fn chatgpt_open_usage() -> Result<(), String> {
    tokio::task::spawn_blocking(|| open::that("https://chatgpt.com/settings/usage"))
        .await
        .map_err(|_| "Unable to open usage settings")?
        .map_err(|_| "Unable to open usage settings".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::{
        pkcs8::{EncodePrivateKey, LineEnding},
        traits::PublicKeyParts,
    };
    use std::sync::LazyLock;
    static SIGNER: LazyLock<(jsonwebtoken::EncodingKey, JwkSet)> = LazyLock::new(|| {
        let key = rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
        let jwks = serde_json::from_value(json!({"keys":[{"kty":"RSA","kid":"fixture","alg":"RS256","use":"sig", "n":URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(key.e().to_bytes_be())}]})).unwrap();
        (
            jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            jwks,
        )
    });
    fn claims() -> Value {
        json!({"iss":ISSUER,"aud":"oaiapp_fixture","sub":"synthetic-subject","iat":now(),"exp":now()+3600,"nonce":"synthetic-nonce","email":"fixture@example.invalid"})
    }
    fn signed(value: &Value) -> String {
        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("fixture".into());
        jsonwebtoken::encode(&header, value, &SIGNER.0).unwrap()
    }
    #[test]
    fn validates_signed_identity_and_rejects_wrong_claims() {
        assert_eq!(
            verify_identity(
                &signed(&claims()),
                "oaiapp_fixture",
                Some("synthetic-nonce"),
                &SIGNER.1
            )
            .unwrap()
            .sub,
            "synthetic-subject"
        );
        for (field, value) in [
            ("iss", json!("https://evil.invalid")),
            ("aud", json!("other-client")),
            ("nonce", json!("other-nonce")),
            ("exp", json!(now() - 60)),
            ("iat", json!(now() + 60)),
            ("sub", json!("")),
        ] {
            let mut c = claims();
            c[field] = value;
            assert!(
                verify_identity(
                    &signed(&c),
                    "oaiapp_fixture",
                    Some("synthetic-nonce"),
                    &SIGNER.1
                )
                .is_err(),
                "{field}"
            );
        }
        let mut missing = claims();
        missing.as_object_mut().unwrap().remove("nonce");
        assert!(verify_identity(
            &signed(&missing),
            "oaiapp_fixture",
            Some("synthetic-nonce"),
            &SIGNER.1
        )
        .is_err());
    }
    #[test]
    fn rejects_tampered_signature_and_untrusted_algorithm() {
        let token = signed(&claims());
        let mut bytes = token.into_bytes();
        let position = bytes.iter().rposition(|b| *b == b'.').unwrap() + 1;
        bytes[position] = if bytes[position] == b'A' { b'B' } else { b'A' };
        assert!(verify_identity(
            std::str::from_utf8(&bytes).unwrap(),
            "oaiapp_fixture",
            Some("synthetic-nonce"),
            &SIGNER.1
        )
        .is_err());
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(Algorithm::HS256),
            &claims(),
            &jsonwebtoken::EncodingKey::from_secret(b"synthetic-test-secret"),
        )
        .unwrap();
        assert!(verify_identity(&token, "oaiapp_fixture", None, &SIGNER.1).is_err());
    }
    fn discovery_fixture() -> Discovery {
        Discovery {
            issuer: ISSUER.into(),
            authorization_endpoint: format!("{ISSUER}/api/accounts/authorize"),
            token_endpoint: String::new(),
            revocation_endpoint: String::new(),
            jwks_uri: String::new(),
        }
    }
    #[test]
    fn registration_has_fresh_pkce_state_nonce_and_correct_loopback() {
        let a = Attempt::new(12345);
        let b = Attempt::new(12345);
        assert_ne!(a.state, b.state);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.verifier, b.verifier);
        let url = a
            .authorization(&discovery_fixture(), "urn:uuid:fixture", None, false)
            .unwrap();
        let values: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(values["client_id"], "dynamic_agent_client");
        assert_eq!(values["agent_name_hint"], AGENT_NAME);
        assert_eq!(
            values["redirect_uri"],
            "http://127.0.0.1:12345/auth/callback"
        );
        assert_eq!(
            values["code_challenge"],
            URL_SAFE_NO_PAD.encode(Sha256::digest(a.verifier.as_bytes()))
        );
        let r = Registration {
            client_id: "oaiapp_fixture".into(),
            subject: Some("synthetic-subject".into()),
            email: None,
            tokens: None,
        };
        let returning = a
            .authorization(&discovery_fixture(), "urn:uuid:fixture", Some(&r), false)
            .unwrap();
        assert!(!returning.query_pairs().any(|(k, _)| k == "agent_name_hint"));
    }
    #[test]
    fn callback_never_uses_dynamic_client_for_exchange() {
        let a = Attempt::new(12345);
        let good = format!(
            "/auth/callback?state={}&code=synthetic-code&client_id=oaiapp_fixture",
            a.state
        );
        assert_eq!(a.callback(&good, None).unwrap().1, "oaiapp_fixture");
        for bad in [
            good.replace("oaiapp_fixture", "dynamic_agent_client"),
            good.replace(&a.state, "wrong-state"),
            good.replace("/auth/callback", "/callback"),
            format!("{good}&state={}", a.state),
            format!("{good}&error=access_denied"),
        ] {
            assert!(a.callback(&bad, None).is_err());
        }
        let r = Registration {
            client_id: "oaiapp_fixture".into(),
            subject: Some("synthetic-subject".into()),
            email: None,
            tokens: None,
        };
        assert!(a
            .callback(
                &format!("/auth/callback?state={}&code=synthetic-code", a.state),
                Some(&r)
            )
            .is_ok());
        assert!(a
            .callback(
                &good.replace("oaiapp_fixture", "different-client"),
                Some(&r)
            )
            .is_err());
    }
    #[test]
    fn identity_only_grant_is_not_plan_consent() {
        let grant: Grant = serde_json::from_value(
            json!({"id_token":"synthetic-id","scope":"openid profile email"}),
        )
        .unwrap();
        let tokens = grant_tokens(grant, None).unwrap();
        assert!(!plan_enabled(&tokens));
        let old = Tokens {
            access: Some("synthetic-access".into()),
            refresh: Some("synthetic-refresh".into()),
            id: Some("synthetic-id".into()),
            scopes: SCOPES.split_whitespace().map(str::to_owned).collect(),
            expires_at: now() + 3600,
            earliest_refresh_at: None,
        };
        assert!(plan_enabled(&old));
        let incomplete = serde_json::from_value(json!({"access_token":"new-synthetic-access","token_type":"Bearer","scope":SCOPES,"expires_in":3600})).unwrap();
        assert!(grant_tokens(incomplete, Some(&old)).is_err());
        let rotated = serde_json::from_value(json!({"access_token":"new-synthetic-access","refresh_token":"new-synthetic-refresh","token_type":"Bearer","scope":SCOPES,"expires_in":3600})).unwrap();
        assert_eq!(
            grant_tokens(rotated, Some(&old))
                .unwrap()
                .refresh
                .as_deref(),
            Some("new-synthetic-refresh")
        );
    }
    #[test]
    fn protected_atomic_store_keeps_host_and_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let host = store.host_id().unwrap();
        assert!(Store::open(dir.path()).is_err());
        let db = Database {
            version: 1,
            active: Some("oaiapp_fixture".into()),
            registrations: vec![Registration {
                client_id: "oaiapp_fixture".into(),
                subject: Some("synthetic-subject".into()),
                email: None,
                tokens: None,
            }],
        };
        store.save(&db).unwrap();
        assert_eq!(store.load().unwrap().active, db.active);
        assert_eq!(store.host_id().unwrap(), host);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(store.path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        atomic_write(&store.path(), b"not json").unwrap();
        assert!(store.load().is_err());
        assert_eq!(store.host_id().unwrap(), host);
    }
    #[test]
    fn missing_host_identity_is_not_silently_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let host = store.host_id().unwrap();
        let db = Database {
            version: 1,
            active: None,
            registrations: vec![Registration {
                client_id: "oaiapp_fixture".into(),
                subject: None,
                email: None,
                tokens: None,
            }],
        };
        store.save(&db).unwrap();
        std::fs::remove_file(dir.path().join("host-id")).unwrap();
        assert!(store
            .host_id()
            .unwrap_err()
            .contains("refusing silent host rotation"));
        assert!(host.starts_with("urn:uuid:"));
    }
    #[test]
    fn sse_handles_fragmented_japanese_and_requires_terminal_success() {
        let data = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"決定\"}\r\n\r\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n";
        let mut s = Sse::default();
        for byte in data.as_bytes() {
            s.push(&[*byte]).unwrap();
        }
        assert_eq!(s.finish().unwrap(), "決定");
        let mut s = Sse::default();
        s.push(data.as_bytes()).unwrap();
        assert_eq!(s.finish().unwrap(), "決定");
        let mut s = Sse::default();
        s.push(b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n")
            .unwrap();
        assert!(s.finish().is_err());
    }
    #[test]
    fn usage_failure_after_stream_start_rejects_draft() {
        let mut s = Sse::default();
        s.push(b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n")
            .unwrap();
        let error = s.push(b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"subscription_sharing_usage_limit_exceeded\"}}}\n\n").unwrap_err();
        assert!(error.starts_with("CHATGPT_USAGE_LIMIT:"));
        let mut s = Sse::default();
        assert!(s
            .push(b"data: {\"type\":\"response.incomplete\"}\n\n")
            .unwrap_err()
            .starts_with("CHATGPT_INCOMPLETE:"));
    }
    #[test]
    fn stream_errors_keep_actual_http_status_and_request_id() {
        let mut s = Sse {
            http_status: 200,
            request_id: "synthetic-request-id".into(),
            ..Sse::default()
        };
        let error = s.push(b"data: {\"type\":\"error\",\"code\":\"subscription_sharing_usage_limit_exceeded\"}\n\n").unwrap_err();
        assert!(error.starts_with("CHATGPT_USAGE_LIMIT:"));
        assert!(error.contains("HTTP 200"));
        assert!(error.contains("synthetic-request-id"));
        assert!(api_error(
            200,
            &json!({"error":{"code":"subscription_sharing_usage_unavailable"}}),
            "fixture"
        )
        .starts_with("CHATGPT_TEMPORARY:"));
    }
    #[test]
    fn public_request_is_preview_compatible_and_models_are_not_cached() {
        let body = responses_body("gpt-6-luna", "synthetic instruction", "synthetic text");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        for key in [
            "temperature",
            "top_p",
            "max_output_tokens",
            "previous_response_id",
            "prompt",
        ] {
            assert!(body.get(key).is_none());
        }
        let list = models(json!({"models":[{"slug":"gpt-6-luna","display_name":"Luna","visibility":"list"},{"slug":"hidden","display_name":"Hidden","visibility":"hidden"}]})).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].slug, "gpt-6-luna");
        assert!(models(json!({"data":[{"id":"gpt-6-luna"}]})).is_err());
        assert!(api_error(
            503,
            &json!({"detail":"temporarily unavailable"}),
            "fixture-request"
        )
        .contains("body_shape=detail"));
    }
    #[test]
    fn completed_output_is_authoritative_and_refusals_are_not_repaired() {
        let mut s = Sse::default();
        s.push(b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[{\"content\":[{\"type\":\"output_text\",\"text\":\"final\"}]}]}}\n\n").unwrap();
        assert_eq!(s.finish().unwrap(), "final");
        let mut s = Sse::default();
        assert!(s
            .push(b"data: {\"type\":\"response.refusal.delta\"}\n\n")
            .unwrap_err()
            .starts_with("CHATGPT_REFUSAL:"));
    }
    #[tokio::test]
    async fn callback_http_headers_can_be_fragmented() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_callback_request(&mut socket).await.unwrap()
        });
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(b"GET /auth/call").await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        socket
            .write_all(b"back?state=synthetic HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        assert!(server
            .await
            .unwrap()
            .starts_with("GET /auth/callback?state=synthetic"));
    }
    #[tokio::test]
    async fn token_exchange_uses_form_encoding_and_surfaces_terminal_refresh_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 2048];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse().unwrap())
                        })
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8(request).unwrap();
            assert!(text.contains("application/x-www-form-urlencoded"));
            assert!(text.contains("client_id=oaiapp_fixture"));
            assert!(text.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
            let body = "{\"error\":\"invalid_grant\"}";
            socket.write_all(format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        });
        let error = exchange(
            &http_client().unwrap(),
            &endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", "oaiapp_fixture"),
                ("refresh_token", "synthetic-refresh"),
                ("resource", RESOURCE),
            ],
        )
        .await
        .err()
        .unwrap();
        assert!(error.starts_with("CHATGPT_SIGN_IN_REQUIRED:"));
        server.await.unwrap();
    }
}
