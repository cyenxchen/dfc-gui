//! DFC GUI Client - Main Entry Point
//!
//! IoT Device Communication Simulator for Wind Turbine Device Function Check

use dfc_gui::app::application::run_app;

fn main() {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting DFC GUI Client...");

    // Run the GPUI application
    run_app();
}
