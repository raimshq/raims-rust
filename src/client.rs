use crate::{contract, Error, ErrorKind};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{
    fmt,
    ops::Deref,
    panic::AssertUnwindSafe,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

/// The origin of the Raims public API in both data environments.
pub const DEFAULT_BASE_URL: &str = "https://api.raims.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(80);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
type Observer = Arc<dyn Fn(RequestDiagnostics) + Send + Sync>;

/// Optional per-call settings. The credential determines its own environment.
#[derive(Clone, Default)]
pub struct RequestOptions {
    pub api_key: Option<String>,
    pub idempotency_key: Option<String>,
    pub timeout: Option<Duration>,
    pub max_read_retries: Option<u8>,
}

impl fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestOptions")
            .field("timeout", &self.timeout)
            .field("max_read_retries", &self.max_read_retries)
            .finish_non_exhaustive()
    }
}

/// Response metadata. Idempotency keys are omitted from default formatting.
#[derive(Clone)]
pub struct LastResponse {
    pub status_code: u16,
    pub request_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub idempotency_replayed: bool,
    pub retry_after: Option<String>,
}

impl fmt::Debug for LastResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LastResponse")
            .field("status_code", &self.status_code)
            .field("request_id", &self.request_id)
            .field("idempotency_replayed", &self.idempotency_replayed)
            .finish_non_exhaustive()
    }
}

impl LastResponse {
    fn from_headers(status_code: u16, headers: &HeaderMap) -> Self {
        Self {
            status_code,
            request_id: header(headers, "request-id").or_else(|| header(headers, "x-request-id")),
            idempotency_key: header(headers, "idempotency-key"),
            idempotency_replayed: header(headers, "idempotency-replayed")
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
            retry_after: header(headers, "retry-after"),
        }
    }
}

/// A typed result together with HTTP metadata and the original JSON value.
#[derive(Clone)]
pub struct ApiResponse<T> {
    pub data: T,
    pub last_response: LastResponse,
    pub raw: Value,
}

impl<T> Deref for ApiResponse<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> fmt::Debug for ApiResponse<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiResponse")
            .field("last_response", &self.last_response)
            .finish_non_exhaustive()
    }
}

/// Request metadata that excludes credentials, resource IDs, queries and bodies.
#[derive(Clone, Debug)]
pub struct RequestDiagnostics {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub attempt: u8,
    pub duration: Duration,
    pub status_code: Option<u16>,
    pub request_id: Option<String>,
    pub will_retry: bool,
    pub retry_delay: Option<Duration>,
}

/// Builds an immutable client that can be shared by concurrent async tasks.
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
    max_read_retries: u8,
    observer: Option<Observer>,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("timeout", &self.timeout)
            .field("max_read_retries", &self.max_read_retries)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// Sets a trusted HTTPS origin. HTTP is accepted only on loopback.
    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = value.into();
        self
    }
    /// Sets the total request budget, including retries and backoff.
    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }
    /// Sets up to five optional GET retries. Writes are always sent once.
    pub fn max_read_retries(mut self, value: u8) -> Self {
        self.max_read_retries = value;
        self
    }
    /// Registers a synchronous observer. Panics do not change request results.
    pub fn request_observer<F: Fn(RequestDiagnostics) + Send + Sync + 'static>(
        mut self,
        value: F,
    ) -> Self {
        self.observer = Some(Arc::new(value));
        self
    }
    /// Validates configuration and initializes the shared connection pool.
    pub fn build(self) -> Result<RaimsClient, Error> {
        visible_ascii(&self.api_key, 4096)?;
        validate_timeout(self.timeout)?;
        validate_retries(self.max_read_retries)?;
        let base_url = validate_base(&self.base_url)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .referer(false)
            .http1_only()
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("raims-rust/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| {
                Error::new(
                    ErrorKind::Connection,
                    "The HTTP transport could not be initialized.",
                )
            })?;
        Ok(RaimsClient {
            inner: Arc::new(Inner {
                api_key: self.api_key,
                base_url,
                http,
                timeout: self.timeout,
                max_read_retries: self.max_read_retries,
                observer: self.observer,
            }),
        })
    }
}

struct Inner {
    api_key: String,
    base_url: reqwest::Url,
    http: reqwest::Client,
    timeout: Duration,
    max_read_retries: u8,
    observer: Option<Observer>,
}

/// A reusable async client for the public Raims API.
#[derive(Clone)]
pub struct RaimsClient {
    inner: Arc<Inner>,
}

impl fmt::Debug for RaimsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RaimsClient")
            .field("version", &env!("CARGO_PKG_VERSION"))
            .finish_non_exhaustive()
    }
}

impl RaimsClient {
    /// Creates a client using the public API origin and no automatic retries.
    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder(api_key).build()
    }
    /// Starts an immutable client configuration.
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: DEFAULT_TIMEOUT,
            max_read_retries: 0,
            observer: None,
        }
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        operation_id: &str,
        path_params: &[(&str, &str)],
        params: &impl Serialize,
        options: &RequestOptions,
    ) -> Result<ApiResponse<T>, Error> {
        let op = contract::operation(operation_id)?;
        let timeout = options.timeout.unwrap_or(self.inner.timeout);
        validate_timeout(timeout)?;
        let retries = options
            .max_read_retries
            .unwrap_or(self.inner.max_read_retries);
        validate_retries(retries)?;
        let prepared = self.prepare(op, path_params, params, options)?;
        let deadline = tokio::time::Instant::now() + timeout;
        self.execute(
            op,
            prepared,
            if op["method"] == "GET" { retries } else { 0 },
            deadline,
        )
        .await
    }

    fn prepare(
        &self,
        op: &Value,
        path_params: &[(&str, &str)],
        params: &impl Serialize,
        options: &RequestOptions,
    ) -> Result<Prepared, Error> {
        let mut path = op["path"]
            .as_str()
            .ok_or_else(|| Error::validation("The operation path is invalid."))?
            .to_owned();
        let declared = op["pathParams"]
            .as_array()
            .ok_or_else(|| Error::validation("The operation path metadata is invalid."))?;
        if path_params.len() != declared.len() {
            return Err(Error::validation(
                "The operation received invalid path parameters.",
            ));
        }
        for name in declared {
            let name = name
                .as_str()
                .ok_or_else(|| Error::validation("The operation path metadata is invalid."))?;
            let matches: Vec<_> = path_params.iter().filter(|(key, _)| *key == name).collect();
            if matches.len() != 1 {
                return Err(Error::validation(
                    "The operation received invalid path parameters.",
                ));
            }
            let value = matches[0].1;
            if value.is_empty()
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            {
                return Err(Error::validation(
                    "Resource identifiers must be non-empty URL-safe strings.",
                ));
            }
            if let Some(schema) = op["pathSchemas"].get(name) {
                contract::validate_input(&Value::String(value.to_owned()), schema)?;
            }
            path = path.replace(&format!("{{{name}}}"), value);
        }
        let value = serde_json::to_value(params)
            .map_err(|_| Error::validation("Request parameters must be valid JSON."))?;
        let fields = value
            .as_object()
            .ok_or_else(|| Error::validation("Request parameters must be a JSON object."))?;
        contract::validate_input(&value, &op["params_schema"])?;
        if op["paginated"] == true
            && fields.get("after").is_some_and(|v| !v.is_null())
            && fields.get("before").is_some_and(|v| !v.is_null())
        {
            return Err(Error::validation(
                "after and before cannot be used together.",
            ));
        }
        let key = options.api_key.as_deref().unwrap_or(&self.inner.api_key);
        visible_ascii(key, 4096)?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| Error::validation("The API key is invalid."))?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(key) = &options.idempotency_key {
            visible_ascii(key, 255)?;
            if op["idempotency"] != true {
                return Err(Error::validation(
                    "This operation does not accept an idempotency key.",
                ));
            }
            let mut key = HeaderValue::from_str(key)
                .map_err(|_| Error::validation("The idempotency key is invalid."))?;
            key.set_sensitive(true);
            headers.insert("idempotency-key", key);
        }
        let mut url = self.inner.base_url.clone();
        url.set_path(&path);
        let body = if op["body"] == true {
            let bytes = serde_json::to_vec(&value)
                .map_err(|_| Error::validation("Request parameters must be valid JSON."))?;
            if op["maxBodyBytes"]
                .as_u64()
                .is_some_and(|n| bytes.len() as u64 > n)
            {
                return Err(Error::validation(
                    "The request body exceeds the published size limit.",
                ));
            }
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Some(bytes)
        } else {
            let mut query = url.query_pairs_mut();
            for (name, value) in fields {
                if op["query"].get(name).is_none() {
                    return Err(Error::validation(
                        "The operation received an unsupported query parameter.",
                    ));
                }
                if value.is_null() {
                    continue;
                }
                let values = value
                    .as_array()
                    .map_or_else(|| vec![value], |v| v.iter().collect());
                for value in values {
                    let text = match value {
                        Value::String(text) => text.clone(),
                        Value::Number(_) | Value::Bool(_) => value.to_string(),
                        _ => {
                            return Err(Error::validation(
                                "Query values must be scalars or lists of scalars.",
                            ))
                        }
                    };
                    query.append_pair(name, &text);
                }
            }
            drop(query);
            if url.query() == Some("") {
                url.set_query(None);
            }
            None
        };
        Ok(Prepared {
            method: reqwest::Method::from_bytes(op["method"].as_str().unwrap_or("").as_bytes())
                .map_err(|_| Error::validation("The operation HTTP method is invalid."))?,
            url,
            headers,
            body,
        })
    }

    async fn execute<T: DeserializeOwned>(
        &self,
        op: &Value,
        prepared: Prepared,
        retries: u8,
        deadline: tokio::time::Instant,
    ) -> Result<ApiResponse<T>, Error> {
        let mut last_response = None;
        for attempt in 0..=retries {
            if tokio::time::Instant::now() >= deadline {
                return Err(timeout_error(last_response.as_ref()));
            }
            let started = Instant::now();
            let mut builder = self
                .inner
                .http
                .request(prepared.method.clone(), prepared.url.clone())
                .headers(prepared.headers.clone());
            if let Some(body) = &prepared.body {
                builder = builder.body(body.clone());
            }
            let received = tokio::time::timeout_at(deadline, builder.send()).await;
            let (info, content_type, payload) = match received {
                Ok(Ok(mut response)) => {
                    let info =
                        LastResponse::from_headers(response.status().as_u16(), response.headers());
                    last_response = Some(info.clone());
                    let content_type = header(response.headers(), "content-type");
                    let mut bytes = Vec::new();
                    let mut read_failed = false;
                    if response
                        .content_length()
                        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
                    {
                        self.notify(op, attempt, started, Some(&info), None);
                        return Err(Error::new(
                            ErrorKind::UnexpectedResponse,
                            "The API response exceeds the supported size.",
                        )
                        .with_response(info));
                    }
                    loop {
                        match tokio::time::timeout_at(deadline, response.chunk()).await {
                            Ok(Ok(Some(chunk))) => {
                                if chunk.len() > MAX_RESPONSE_BYTES - bytes.len() {
                                    self.notify(op, attempt, started, Some(&info), None);
                                    return Err(Error::new(
                                        ErrorKind::UnexpectedResponse,
                                        "The API response exceeds the supported size.",
                                    )
                                    .with_response(info));
                                }
                                bytes.extend_from_slice(&chunk);
                            }
                            Ok(Ok(None)) => break,
                            Ok(Err(_)) => {
                                read_failed = true;
                                break;
                            }
                            Err(_) => {
                                self.notify(op, attempt, started, Some(&info), None);
                                return Err(timeout_error(Some(&info)));
                            }
                        }
                    }
                    if read_failed {
                        let delay = if attempt < retries {
                            retry_delay(attempt, Some(&info))
                        } else {
                            None
                        };
                        self.notify(op, attempt, started, Some(&info), delay);
                        if let Some(delay) = delay {
                            wait_for_retry(deadline, delay, Some(&info)).await?;
                            continue;
                        }
                        return Err(Error::new(
                            ErrorKind::Connection,
                            "The API connection failed or timed out.",
                        )
                        .with_response(info));
                    }
                    (info, content_type, bytes)
                }
                Ok(Err(error)) => {
                    let kind = if error.is_timeout() {
                        ErrorKind::Timeout
                    } else {
                        ErrorKind::Connection
                    };
                    let delay = if attempt < retries {
                        retry_delay(attempt, None)
                    } else {
                        None
                    };
                    self.notify(op, attempt, started, None, delay);
                    if let Some(delay) = delay {
                        wait_for_retry(deadline, delay, last_response.as_ref()).await?;
                        continue;
                    }
                    return Err(Error::new(kind, "The API connection failed or timed out."));
                }
                Err(_) => {
                    self.notify(op, attempt, started, None, None);
                    return Err(timeout_error(last_response.as_ref()));
                }
            };
            let delay = if attempt < retries
                && [408, 429, 500, 502, 503, 504].contains(&info.status_code)
            {
                retry_delay(attempt, Some(&info))
            } else {
                None
            };
            self.notify(op, attempt, started, Some(&info), delay);
            if let Some(delay) = delay {
                wait_for_retry(deadline, delay, Some(&info)).await?;
                continue;
            }
            return interpret(op, info, content_type.as_deref(), &payload);
        }
        Err(Error::new(
            ErrorKind::Connection,
            "The API connection failed or timed out.",
        ))
    }

    fn notify(
        &self,
        op: &Value,
        attempt: u8,
        started: Instant,
        info: Option<&LastResponse>,
        delay: Option<Duration>,
    ) {
        if let Some(observer) = &self.inner.observer {
            let diagnostics = RequestDiagnostics {
                operation_id: op["id"].as_str().unwrap_or("").to_owned(),
                method: op["method"].as_str().unwrap_or("").to_owned(),
                path: op["path"].as_str().unwrap_or("").to_owned(),
                attempt: attempt + 1,
                duration: started.elapsed(),
                status_code: info.map(|r| r.status_code),
                request_id: info.and_then(|r| r.request_id.clone()),
                will_retry: delay.is_some(),
                retry_delay: delay,
            };
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| observer(diagnostics)));
        }
    }
}

fn timeout_error(info: Option<&LastResponse>) -> Error {
    let error = Error::new(
        ErrorKind::Timeout,
        "The API connection failed or timed out.",
    );
    match info {
        Some(info) => error.with_response(info.clone()),
        None => error,
    }
}

async fn wait_for_retry(
    deadline: tokio::time::Instant,
    delay: Duration,
    last_response: Option<&LastResponse>,
) -> Result<(), Error> {
    tokio::time::timeout_at(deadline, tokio::time::sleep(delay))
        .await
        .map_err(|_| timeout_error(last_response))
}

struct Prepared {
    method: reqwest::Method,
    url: reqwest::Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
}

fn interpret<T: DeserializeOwned>(
    op: &Value,
    info: LastResponse,
    content_type: Option<&str>,
    payload: &[u8],
) -> Result<ApiResponse<T>, Error> {
    let unexpected = || {
        Error::new(
            ErrorKind::UnexpectedResponse,
            "The API returned an unexpected response.",
        )
        .with_response(info.clone())
    };
    if (300..400).contains(&info.status_code) {
        return Err(unexpected());
    }
    if !(200..300).contains(&info.status_code) {
        let kind = match info.status_code {
            400 | 405 | 413 | 415 | 422 => ErrorKind::InvalidRequest,
            401 => ErrorKind::Authentication,
            403 => ErrorKind::Permission,
            404 => ErrorKind::NotFound,
            409 => ErrorKind::Conflict,
            429 => ErrorKind::RateLimit,
            _ => ErrorKind::Api,
        };
        let api_error = serde_json::from_slice::<Value>(payload)
            .ok()
            .and_then(|v| v.get("error").filter(|v| v.is_object()).cloned());
        return Err(Error::new(kind, "The API request failed.")
            .with_response(info)
            .with_api_error(api_error));
    }
    let schema = op["responses"]
        .get(info.status_code.to_string())
        .ok_or_else(unexpected)?;
    let raw = if schema.is_null() {
        if !payload.is_empty() {
            return Err(unexpected());
        }
        Value::Null
    } else {
        if let Some(content_type) = content_type {
            let media_type = content_type
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if media_type != "application/json"
                && !(media_type.starts_with("application/") && media_type.ends_with("+json"))
            {
                return Err(unexpected());
            }
        }
        let value: Value = serde_json::from_slice(payload).map_err(|_| unexpected())?;
        contract::validate_response(&value, schema).map_err(|e| e.with_response(info.clone()))?;
        value
    };
    let data = serde_json::from_value(raw.clone()).map_err(|_| unexpected())?;
    Ok(ApiResponse {
        data,
        last_response: info,
        raw,
    })
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value.to_string())
}

fn visible_ascii(value: &str, maximum: usize) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > maximum
        || !value.bytes().all(|b| (0x21..=0x7e).contains(&b))
    {
        return Err(Error::validation(
            "Credentials and idempotency keys must contain visible ASCII without spaces.",
        ));
    }
    Ok(())
}

fn validate_timeout(timeout: Duration) -> Result<(), Error> {
    if timeout.is_zero() || timeout > Duration::from_secs(300) {
        return Err(Error::validation(
            "The timeout must be greater than zero and at most 300 seconds.",
        ));
    }
    Ok(())
}

fn validate_retries(retries: u8) -> Result<(), Error> {
    if retries > 5 {
        return Err(Error::validation(
            "The maximum read retries must be between zero and five.",
        ));
    }
    Ok(())
}

fn validate_base(value: &str) -> Result<reqwest::Url, Error> {
    let fail = || {
        Error::validation("The base URL must be an HTTPS origin without credentials, path, query or fragment; HTTP is accepted only on loopback.")
    };
    if value.bytes().any(|b| b <= 0x20 || b == 0x7f || b == b'\\')
        || value.contains('?')
        || value.contains('#')
        || value.contains('@')
    {
        return Err(fail());
    }
    let (_, raw_authority) = value.split_once("://").ok_or_else(fail)?;
    let raw_authority = raw_authority.strip_suffix('/').unwrap_or(raw_authority);
    if raw_authority.is_empty() || raw_authority.contains('/') {
        return Err(fail());
    }
    let url = reqwest::Url::parse(value).map_err(|_| fail())?;
    let host = url.host_str().ok_or_else(fail)?;
    let loopback = host == "localhost"
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|v| v.is_loopback());
    if !url.username().is_empty()
        || url.password().is_some()
        || !["", "/"].contains(&url.path())
        || url.port() == Some(0)
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
    {
        return Err(fail());
    }
    Ok(url)
}

fn retry_delay(attempt: u8, info: Option<&LastResponse>) -> Option<Duration> {
    let baseline = Duration::from_millis(250 * (1_u64 << attempt.min(3)));
    let advice = match info.and_then(|v| v.retry_after.as_deref()) {
        Some(value) => {
            let value = value.trim();
            if value.bytes().all(|b| b.is_ascii_digit()) && !value.is_empty() {
                Duration::from_secs(value.parse::<u64>().ok()?)
            } else {
                httpdate::parse_http_date(value)
                    .ok()
                    .map(|t| t.duration_since(SystemTime::now()).unwrap_or_default())
                    .unwrap_or_default()
            }
        }
        None => Duration::ZERO,
    };
    if advice > Duration::from_secs(30) {
        return None;
    }
    Some(baseline.max(advice))
}
