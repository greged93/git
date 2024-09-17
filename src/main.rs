use clap::{Parser, Subcommand};
use eyre::eyre;
use std::fs;
use std::io::Read;

#[derive(Parser)]
pub struct Args {
    #[clap(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init,
    CatFile {
        #[clap(short = 'p', long = "path")]
        sha: String,
    },
}

fn main() -> eyre::Result<()> {
    // Uncomment this block to pass the first stage
    let args = Args::parse();
    match args.subcommand {
        Command::Init => {
            // Create the git structure
            fs::create_dir(".git")?;
            fs::create_dir(".git/objects")?;
            fs::create_dir(".git/refs")?;
            fs::write(".git/HEAD", "ref: refs/heads/main\n")?;

            println!("Initialized git directory");
            Ok(())
        }
        Command::CatFile { sha } => {
            // Read the file and start the decoder
            let path = format!(".git/objects/{}/{}", &sha[..2], &sha[2..]);
            let compressed = fs::read(path)?;
            let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);

            // Decode the compressed file to a string
            let mut s = String::new();
            decoder.read_to_string(&mut s)?;

            // Read the length
            let zero_byte_pos = s.find('\0').ok_or(eyre!("missing \0 byte"))?;
            let content = &s[zero_byte_pos + 1..];

            print!("{}", content);

            Ok(())
        }
    }
}
