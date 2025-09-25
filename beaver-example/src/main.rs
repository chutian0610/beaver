use beaver_bootstrap::{bootstrap::Bootstrap, error::BootstrapError};

fn main() -> Result<(), BootstrapError> {
    let mut bootstrap = Bootstrap::builder()
        .initialize_logging(true)
        .show_config(true)
        .modules(vec![])
        .build();
    bootstrap.initialize()?;
    tracing::info!("bootstrap initialized");
    Ok(())
}
