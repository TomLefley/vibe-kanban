use std::sync::OnceLock;
use std::env;

use sentry_tracing::{EventFilter, SentryLayer};
use tracing::{info, Level};

static INIT_GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();
static SENTRY_ENABLED: OnceLock<bool> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub enum SentrySource {
    Backend,
    Mcp,
}

impl SentrySource {
    fn tag(self) -> &'static str {
        match self {
            SentrySource::Backend => "backend",
            SentrySource::Mcp => "mcp",
        }
    }
}

fn environment() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "production"
    }
}

pub fn init_once(source: SentrySource) {
    // Only initialize Sentry if DSN is explicitly provided via environment variable
    let dsn = match env::var("SENTRY_DSN") {
        Ok(dsn) if !dsn.is_empty() => dsn,
        _ => {
            SENTRY_ENABLED.get_or_init(|| false);
            info!("Sentry DSN not configured. Error reporting disabled.");
            return;
        }
    };

    SENTRY_ENABLED.get_or_init(|| true);

    INIT_GUARD.get_or_init(|| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some(environment().into()),
                ..Default::default()
            },
        ))
    });

    sentry::configure_scope(|scope| {
        scope.set_tag("source", source.tag());
    });
}

pub fn configure_user_scope(user_id: &str, username: Option<&str>, email: Option<&str>) {
    let mut sentry_user = sentry::User {
        id: Some(user_id.to_string()),
        ..Default::default()
    };

    if let Some(username) = username {
        sentry_user.username = Some(username.to_string());
    }

    if let Some(email) = email {
        sentry_user.email = Some(email.to_string());
    }

    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry_user));
    });
}

pub fn sentry_layer<S>() -> SentryLayer<S>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    SentryLayer::default()
        .span_filter(|meta| {
            matches!(
                *meta.level(),
                Level::DEBUG | Level::INFO | Level::WARN | Level::ERROR
            )
        })
        .event_filter(|meta| match *meta.level() {
            Level::ERROR => EventFilter::Event,
            Level::DEBUG | Level::INFO | Level::WARN => EventFilter::Breadcrumb,
            Level::TRACE => EventFilter::Ignore,
        })
}
