mod git;

use crate::git::GitFile;
use clap::{Parser, Subcommand};
use sha1::Digest;
use std::fs;
use std::io::Write;
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
    WriteTree,
    CommitTree {
        tree_sha: String,
        #[clap(short)]
        parent_sha: String,
        #[clap(short)]
        message: String,
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
        Command::WriteTree => {
            let file = GitFile::from_directory(PathBuf::from("."))?;

            // Write the compressed data to output
            let hash = hex::encode(&file.sha);
            let base_path = format!(".git/objects/{}", &hash[..2]);
            let output_path = format!("{}/{}", base_path, &hash[2..]);
            let _ = fs::create_dir(base_path);
            fs::write(output_path, file.compress()?)?;

            println!("{}", hash);
            Ok(())
        }
        Command::CommitTree {
            parent_sha,
            message,
            tree_sha,
        } => {
            let content = format!(
                "tree {tree_sha}\nparent {parent_sha}\nauthor Greg <greg@notyourbusiness.com +0000\n\n{message}\n"
            );
            let content = content.as_bytes();
            let header = format!("commit {}\0", content.len());

            let commit = [header.as_bytes(), content].concat();

            // Hash the git file
            let mut hasher = sha1::Sha1::new();
            hasher.update(&commit);
            let hash = hasher.finalize();
            let hash = hex::encode(hash);

            // Compress the file
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Default::default());

            encoder.write_all(&commit)?;
            let content = encoder.finish()?;

            let base_path = format!(".git/objects/{}", &hash[..2]);
            let output_path = format!("{}/{}", base_path, &hash[2..]);
            let _ = fs::create_dir(base_path);
            fs::write(output_path, content)?;

            println!("{}", hash);

            Ok(())
        }
    }
}
