use beaver_bootstrap::{bootstrap::Bootstrap, error::BootstrapError};

fn main() -> Result<(), BootstrapError> {
    let bootstrap = Bootstrap::builder()
        .initialize_logging(true)
        .modules(vec![])
        .build();
    bootstrap.initialize()?;
    tracing::info!("bootstrap initialized");
    Ok(())
}
