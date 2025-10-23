use restream::adapter::{FirefliesBridge, FirefliesConfig};
use restream::consts::WEBHOOK_URL_PROD;
use std::str::FromStr;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Directive;

fn create_log_filter() -> Result<EnvFilter, tracing_subscriber::filter::ParseError> {
    let filter = EnvFilter::new("info")
        .add_directive(Directive::from_str("aws_config::profile::credentials=off")?)
        .add_directive(Directive::from_str("sqlx::query=off")?)
        .add_directive(Directive::from_str("sqlx::postgres::notice=off")?);
    Ok(filter)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let filter = create_log_filter().unwrap_or_else(|err| {
        eprintln!("Failed to parse tracing directives {err}. Falling back to 'info'.",);
        EnvFilter::new("info")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(true)
        .init();

    info!("Start testing subscriber");

    let config = FirefliesConfig {
        api_token: "".to_string(),
        transcript_id: "".to_string(),
        webhook_url: WEBHOOK_URL_PROD.to_string(),
    };

    let Ok((bridge, mut receiver)) = FirefliesBridge::new(config) else {
        panic!("Failed to initialize FirefliesBridge");
    };

    let result = bridge.start().await;
    info!("Bridge started: {:?}", result);
    loop {
        let init_result = receiver.recv().await;
        info!("{:?}", init_result);
    }

    Ok(())
}
