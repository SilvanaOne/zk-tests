use chrono::Local;
use lambda::handler::function_handler;
use lambda_runtime::{Error, run, service_fn};
use tracing::{Level, Subscriber};
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{self, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

/// Custom formatter that doesn't include span context but adds timestamp and module
struct SimpleFmt;

impl<S, N> FormatEvent<S, N> for SimpleFmt
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // Write timestamp
        write!(writer, "{} ", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))?;

        // Write level
        let level = event.metadata().level();
        match *level {
            Level::ERROR => write!(writer, "ERROR ")?,
            Level::WARN => write!(writer, "WARN  ")?,
            Level::INFO => write!(writer, "INFO  ")?,
            Level::DEBUG => write!(writer, "DEBUG ")?,
            Level::TRACE => write!(writer, "TRACE ")?,
        }

        // Write full module path
        let target = event.metadata().target();
        // Show full module path for better debugging
        write!(writer, "[{}] ", target)?;

        // Write the message without any span context
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Disable colors
    unsafe {
        std::env::set_var("NO_COLOR", "1");
        std::env::set_var("TERM", "dumb");
    }

    // Create subscriber with custom formatter
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .event_format(SimpleFmt)
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry().with(filter).with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| {
            eprintln!("Failed to set tracing subscriber: {}", e);
            Error::from(format!("Failed to set tracing subscriber: {}", e))
        })?;

    run(service_fn(function_handler)).await
}
