use base64::prelude::{BASE64_URL_SAFE, Engine as _};
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use google_gmail1::{
    Gmail,
    api::{MessagePart, Scope},
    hyper,
    hyper::client::HttpConnector,
    hyper_rustls,
    oauth2::{self, ServiceAccountKey},
};
use html2text::from_read;
use sr_common::db::{DbPoolError, PgPool, create_pool_from_url};
use std::collections::HashSet;
use tokio::time::{Duration, interval};
use tracing::{debug, info, warn};

#[derive(Debug, Parser)]
#[command(
    name = "sr-gmail-ingestor",
    about = "Ingest emails from Gmail directly via Google Cloud"
)]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    db_url: String,

    /// Path to the Google service account JSON key (with Gmail scopes enabled)
    #[arg(long, env = "GWS_SERVICE_ACCOUNT_KEY")]
    sa_key_path: String,

    /// User to impersonate via Domain-Wide Delegation
    #[arg(long, env = "GWS_IMPERSONATE_USER")]
    impersonate_user: String,

    /// Poll interval in seconds
    #[arg(long, env = "GWS_POLL_INTERVAL_SECONDS", default_value_t = 60)]
    poll_interval: u64,

    /// Gmail search query for project/案件 emails (no attachments expected)
    #[arg(
        long,
        env = "GWS_ANKEN_QUERY",
        default_value = "label:partner -has:attachment"
    )]
    anken_query: String,

    /// Gmail search query for talent/人材 emails (usually with attachments)
    #[arg(
        long,
        env = "GWS_JINZAI_QUERY",
        default_value = "label:partner has:attachment"
    )]
    jinzai_query: String,
}

#[derive(Debug, Clone, Copy)]
enum EmailType {
    Anken,
    Jinzai,
}

#[derive(Debug, Default)]
struct EmailData {
    message_id: String,
    thread_id: Option<String>,
    sender_address: Option<String>,
    sender_name: Option<String>,
    subject: Option<String>,
    body_text: Option<String>,
    received_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
enum IngestError {
    #[error("failed to load service account key: {0}")]
    ServiceAccountLoad(std::io::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("oauth error: {0}")]
    Oauth(#[from] oauth2::Error),
    #[error("gmail api error: {0}")]
    Gmail(#[from] google_gmail1::Error),
    #[error("database pool error: {0}")]
    DbPool(#[from] DbPoolError),
    #[error("postgres pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("utf8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("html to text conversion failed: {0}")]
    HtmlToText(#[from] html2text::Error),
}

struct GmailIngestor {
    gmail: Gmail<hyper_rustls::HttpsConnector<HttpConnector>>,
    pool: PgPool,
    anken_query: String,
    jinzai_query: String,
}

impl GmailIngestor {
    async fn new(cli: &Cli, pool: PgPool) -> Result<Self, IngestError> {
        let key: ServiceAccountKey = oauth2::read_service_account_key(&cli.sa_key_path)
            .await
            .map_err(IngestError::ServiceAccountLoad)?;
        let auth = oauth2::ServiceAccountAuthenticator::builder(key)
            .subject(cli.impersonate_user.clone())
            .build()
            .await?;

        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()?
            .https_only()
            .enable_http1()
            .enable_http2()
            .build();
        let client = hyper::Client::builder().build::<_, hyper::Body>(https);

        let gmail = Gmail::new(client, auth);

        Ok(Self {
            gmail,
            pool,
            anken_query: cli.anken_query.clone(),
            jinzai_query: cli.jinzai_query.clone(),
        })
    }

    async fn poll_once(&mut self) -> Result<u32, IngestError> {
        let mut processed = 0;

        let anken_query = self.anken_query.clone();
        processed += self
            .fetch_and_store_emails(&anken_query, EmailType::Anken)
            .await?;

        let jinzai_query = self.jinzai_query.clone();
        processed += self
            .fetch_and_store_emails(&jinzai_query, EmailType::Jinzai)
            .await?;

        Ok(processed)
    }

    async fn fetch_and_store_emails(
        &mut self,
        query: &str,
        email_type: EmailType,
    ) -> Result<u32, IngestError> {
        let mut processed = 0u32;
        let user_id = "me";

        let mut page_token: Option<String> = None;

        loop {
            let mut list_call = self
                .gmail
                .users()
                .messages_list(user_id)
                .q(query)
                .max_results(100)
                .add_scope(Scope::Readonly);

            if let Some(token) = page_token.as_deref() {
                list_call = list_call.page_token(token);
            }

            let (_, list_response) = list_call.doit().await?;

            let messages = list_response.messages.unwrap_or_default();
            let message_ids: Vec<String> = messages
                .iter()
                .filter_map(|msg| msg.id.clone())
                .collect();

            let already_seen = self.fetch_existing_ids(&message_ids).await?;

            for msg_ref in messages {
                let msg_id = match msg_ref.id.clone() {
                    Some(id) => id,
                    None => continue,
                };

                if already_seen.contains(&msg_id) {
                    debug!(message_id = %msg_id, "already ingested, skipping");
                    continue;
                }

                let (_, message) = self
                    .gmail
                    .users()
                    .messages_get(user_id, &msg_id)
                    .format("full")
                    .add_scope(Scope::Readonly)
                    .doit()
                    .await?;

                let mut email = self.parse_message(message)?;
                email.message_id = msg_id.clone();

                match email_type {
                    EmailType::Anken => self.store_anken_email(&email).await?,
                    EmailType::Jinzai => self.store_jinzai_email(&email).await?,
                }

                processed += 1;
            }

            page_token = list_response.next_page_token;

            if page_token.is_none() {
                break;
            }
        }

        Ok(processed)
    }

    fn parse_message(
        &self,
        message: google_gmail1::api::Message,
    ) -> Result<EmailData, IngestError> {
        let payload = message.payload.unwrap_or_default();
        let thread_id = message.thread_id;
        let subject = self.header_value(&payload, "Subject");
        let from = self.header_value(&payload, "From");
        let received_at = self.extract_received_at(&payload, message.internal_date)?;
        let body_text = self.extract_body(&payload)?;

        let (sender_name, sender_address) = parse_sender(&from);

        Ok(EmailData {
            message_id: message.id.unwrap_or_default(),
            thread_id,
            sender_address,
            sender_name,
            subject,
            body_text,
            received_at,
        })
    }

    fn header_value(&self, payload: &MessagePart, name: &str) -> Option<String> {
        payload
            .headers
            .as_ref()
            .and_then(|headers| {
                headers.iter().find_map(|h| {
                    let header_name = h.name.as_deref()?;
                    if header_name.eq_ignore_ascii_case(name) {
                        h.value.clone()
                    } else {
                        None
                    }
                })
            })
            .map(|v| v.trim().to_string())
    }

    fn extract_received_at(
        &self,
        payload: &MessagePart,
        internal_date: Option<i64>,
    ) -> Result<Option<DateTime<Utc>>, IngestError> {
        if let Some(date_header) = self.header_value(payload, "Date") {
            if let Ok(parsed) = DateTime::parse_from_rfc2822(&date_header) {
                return Ok(Some(parsed.with_timezone(&Utc)));
            } else {
                warn!(header = %date_header, "failed to parse Date header, falling back to internalDate");
            }
        }

        if let Some(ms) = internal_date {
            if let Some(ts) = DateTime::from_timestamp_millis(ms) {
                return Ok(Some(ts));
            }
        }

        Ok(None)
    }

    fn extract_body(&self, payload: &MessagePart) -> Result<Option<String>, IngestError> {
        if let Some(part) = self.find_part_with_mime(payload, "text/plain") {
            if let Some(body) = decode_part_body(part)? {
                return Ok(Some(body));
            }
        }

        if let Some(part) = self.find_part_with_mime(payload, "text/html") {
            if let Some(body) = decode_part_body(part)? {
                let text = from_read(body.as_bytes(), usize::MAX)?;
                return Ok(Some(text.trim().to_string()));
            }
        }

        if let Some(body) = decode_part_body(payload)? {
            return Ok(Some(body));
        }

        Ok(None)
    }

    fn find_part_with_mime<'a>(
        &self,
        part: &'a MessagePart,
        target: &str,
    ) -> Option<&'a MessagePart> {
        if let Some(mime) = &part.mime_type {
            if mime.eq_ignore_ascii_case(target) {
                return Some(part);
            }
        }

        if let Some(parts) = &part.parts {
            for child in parts {
                if let Some(found) = self.find_part_with_mime(child, target) {
                    return Some(found);
                }
            }
        }

        None
    }

    async fn fetch_existing_ids(
        &self,
        message_ids: &[String],
    ) -> Result<HashSet<String>, IngestError> {
        if message_ids.is_empty() {
            return Ok(HashSet::new());
        }

        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT message_id FROM ses.anken_emails WHERE message_id = ANY($1)\n                 UNION\n                 SELECT message_id FROM ses.jinzai_emails WHERE message_id = ANY($1)",
                &[&message_ids],
            )
            .await?;

        let existing = rows
            .into_iter()
            .map(|row| row.get::<_, String>(0))
            .collect();

        Ok(existing)
    }

    async fn store_anken_email(&self, email: &EmailData) -> Result<(), IngestError> {
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
                INSERT INTO ses.anken_emails (
                    message_id, sender_address, sender_name, subject,
                    body_text, received_at, thread_id
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (message_id) DO NOTHING
                "#,
                &[
                    &email.message_id,
                    &email.sender_address,
                    &email.sender_name,
                    &email.subject,
                    &email.body_text,
                    &email.received_at,
                    &email.thread_id,
                ],
            )
            .await?;
        Ok(())
    }

    async fn store_jinzai_email(&self, email: &EmailData) -> Result<(), IngestError> {
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
                INSERT INTO ses.jinzai_emails (
                    message_id, sender_address, sender_name, subject,
                    body_text, received_at, thread_id, skillsheet_url
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (message_id) DO NOTHING
                "#,
                &[
                    &email.message_id,
                    &email.sender_address,
                    &email.sender_name,
                    &email.subject,
                    &email.body_text,
                    &email.received_at,
                    &email.thread_id,
                    &None::<String>,
                ],
            )
            .await?;
        Ok(())
    }
}

fn parse_sender(header: &Option<String>) -> (Option<String>, Option<String>) {
    if let Some(raw) = header {
        if let Ok(addrs) = mailparse::addrparse(raw) {
            if let Some(addr) = addrs.get(0) {
                match addr {
                    mailparse::MailAddr::Single(info) => {
                        return (info.display_name.clone(), Some(info.addr.clone()));
                    }
                    mailparse::MailAddr::Group(group) => {
                        if let Some(first) = group.addrs.first() {
                            return (first.display_name.clone(), Some(first.addr.clone()));
                        }
                    }
                }
            }
        }
    }

    (None, None)
}

fn decode_part_body(part: &MessagePart) -> Result<Option<String>, IngestError> {
    if let Some(body) = &part.body {
        if let Some(data) = &body.data {
            let bytes = BASE64_URL_SAFE.decode(data)?;
            return Ok(Some(String::from_utf8(bytes)?));
        }
    }

    Ok(None)
}

async fn run() -> Result<(), IngestError> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let pool = create_pool_from_url(&cli.db_url)?;

    let mut ingestor = GmailIngestor::new(&cli, pool).await?;

    info!(poll_interval = cli.poll_interval, "starting Gmail ingestor");
    let mut ticker = interval(Duration::from_secs(cli.poll_interval));

    loop {
        ticker.tick().await;

        match ingestor.poll_once().await {
            Ok(processed) => {
                if processed > 0 {
                    info!(processed, "ingested gmail messages");
                } else {
                    debug!("no new Gmail messages in this cycle");
                }
            }
            Err(err) => warn!(error = %err, "poll failed"),
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("sr-gmail-ingestor failed: {err}");
        std::process::exit(1);
    }
}
