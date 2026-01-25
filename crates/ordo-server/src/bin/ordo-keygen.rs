use clap::Parser;
use ordo_core::signature::signer::RuleSigner;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "ordo-keygen",
    about = "Generate Ed25519 keypair for Ordo rule signing"
)]
struct Args {
    /// Output directory for key files
    #[arg(long, default_value = "./keys")]
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.output)?;

    let (public_key, private_key) = RuleSigner::generate_keypair();
    let public_path = args.output.join("public.key");
    let private_path = args.output.join("private.key");

    fs::write(&public_path, format!("{}\n", public_key))?;
    fs::write(&private_path, format!("{}\n", private_key))?;

    println!("Public key: {}", public_path.display());
    println!("Private key: {}", private_path.display());
    Ok(())
}
