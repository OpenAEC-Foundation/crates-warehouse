use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openaec-cli")]
#[command(about = "OpenAEC Report Generator CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a PDF report from JSON data
    Generate {
        /// Path to JSON report data
        #[arg(short, long)]
        data: String,

        /// Output PDF path
        #[arg(short, long)]
        output: String,

        /// Tenant directory (overrides OPENAEC_TENANT_DIR)
        #[arg(long)]
        tenant_dir: Option<String>,
    },

    /// Validate JSON report data against the schema
    Validate {
        /// Path to JSON report data
        #[arg(short, long)]
        data: String,
    },

    /// Start the API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8000")]
        port: u16,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            data,
            output,
            tenant_dir,
        } => {
            // Set tenant dir env var if provided
            if let Some(dir) = tenant_dir {
                // SAFETY: single-threaded at this point, no other threads reading env vars
                unsafe { std::env::set_var("OPENAEC_TENANT_DIR", &dir) };
            }

            // Read JSON
            let json_str = std::fs::read_to_string(&data).unwrap_or_else(|e| {
                eprintln!("Error reading {}: {}", data, e);
                std::process::exit(1);
            });

            // Parse
            let report_data = openaec_core::ReportData::from_json(&json_str).unwrap_or_else(|e| {
                eprintln!("Error parsing JSON: {}", e);
                std::process::exit(1);
            });

            // Generate
            let pdf_bytes = openaec_core::engine::generate_pdf(
                &report_data,
                std::path::Path::new(&output),
            )
            .unwrap_or_else(|e| {
                eprintln!("Error generating PDF: {}", e);
                std::process::exit(1);
            });

            // Write output
            std::fs::write(&output, &pdf_bytes).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", output, e);
                std::process::exit(1);
            });

            println!("Generated: {} ({} bytes)", output, pdf_bytes.len());
        }
        Commands::Validate { data } => {
            let json_str = std::fs::read_to_string(&data).unwrap_or_else(|e| {
                eprintln!("Error reading {}: {}", data, e);
                std::process::exit(1);
            });

            match openaec_core::ReportData::from_json(&json_str) {
                Ok(report) => {
                    println!(
                        "Valid: {} sections, template={}",
                        report.sections.len(),
                        report.template
                    );
                }
                Err(e) => {
                    eprintln!("Invalid: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Serve { port } => {
            println!("Starting server on port {}...", port);
            // TODO: Wire up openaec-server as a library call or spawn process
        }
    }
}
