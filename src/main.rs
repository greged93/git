mod git;

use crate::git::GitFile;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Args {
    #[clap(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init,
    // Reads the content of the file at sha
    CatFile {
        #[clap(short = 'p', long = "path")]
        sha: String,
    },
    HashObject {
        #[clap(short = 'w', long = "write")]
        path: PathBuf,
    },
    LsTree {
        #[clap(long)]
        name_only: bool,
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
            let git_file = GitFile::new(sha)?;

            print!("{}", git_file);
            Ok(())
        }
        Command::HashObject { path } => {
            // Read the file at the given path
            let file = GitFile::from_file(path)?;

            // Get the hash
            let hash = hex::encode(file.hash());

            // Compress the file
            let compressed = file.compress()?;

            // Write the compressed data to output
            let base_path = format!(".git/objects/{}", &hash[..2]);
            let output_path = format!("{}/{}", base_path, &hash[2..]);
            let _ = fs::create_dir(base_path);
            fs::write(output_path, compressed)?;

            print!("{}", hash);
            Ok(())
        }
        Command::LsTree { sha, .. } => {
            let file = GitFile::new(sha)?;

            print!("{}", file);
            Ok(())
        }
    }
}
