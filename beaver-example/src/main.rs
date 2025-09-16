use beaver_bootstrap::{bootstrap::Bootstrap, error::BootstrapError};

fn main() -> Result<(), BootstrapError> {
    let bootstrap = Bootstrap::builder().build();
    bootstrap.initialize()?;
    Ok(())
}
