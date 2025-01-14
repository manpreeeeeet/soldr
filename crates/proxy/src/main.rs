use anyhow::Result;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soldr=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let config = match args.config_path {
        Some(path) => read_config(&path)?,
        None => soldr::Config::default(),
    };

    let (ingest, mgmt, retry_queue) = soldr::app(&config).await?;

    let mgmt_listener = config.management_listener.parse()?;
    let ingest_listener = config.ingest_listener.parse()?;

    tokio::spawn(async move {
        tracing::info!("management API listening on {}", mgmt_listener);
        if let Err(err) = axum_server::bind(mgmt_listener)
            .serve(mgmt.into_make_service())
            .await
        {
            eprintln!("Failed to start management API server: {}", err);
        }
    });

    tokio::spawn(async move {
        tracing::info!("starting retry queue");
        retry_queue.start().await;
    });

    let tls_config = if config.tls.enable {
        let cert_path = config.tls.cert_path.unwrap();
        let key_path = config.tls.key_path.unwrap();
        Some(RustlsConfig::from_pem_file(cert_path, key_path).await?)
    } else {
        None
    };

    tracing::info!("ingest listening on {}", ingest_listener);
    if let Some(tls_config) = tls_config {
        tracing::info!("tls configured for {}", ingest_listener);
        axum_server::bind_rustls(ingest_listener, tls_config)
            .serve(ingest.into_make_service())
            .await?;
    } else {
        axum_server::bind(ingest_listener)
            .serve(ingest.into_make_service())
            .await?;
    }

    Ok(())
}

fn read_config(config_path: &str) -> Result<soldr::Config> {
    let content = std::fs::read_to_string(config_path)?;
    Ok(toml::from_str(&content)?)
}
