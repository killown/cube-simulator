use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "WGPU Cube Simulator")]
pub struct Args {
    #[arg(short, long, default_value_t = 6)]
    pub cubes: u32,
    #[arg(short = 'z', long, default_value_t = 0.5)]
    pub size: f32,
    #[arg(short, long, default_value_t = 1.0)]
    pub speed: f32,
    #[arg(long, default_value_t = 0.5)]
    pub red: f32,
    #[arg(long, default_value_t = 0.8)]
    pub green: f32,
    #[arg(long, default_value_t = 0.2)]
    pub blue: f32,
    #[arg(short = 't', long, default_value_t = 25.0)]
    pub threshold: f32,
    #[arg(short = 'f', long)]
    pub format: Option<String>,
    #[arg(short = 'm', long)]
    pub mode: Option<String>,
    #[arg(long, default_value_t = 80)]
    pub steps: u32,
    #[arg(long)]
    pub csv: Option<String>,
    #[arg(long)]
    pub json: Option<String>,
}
