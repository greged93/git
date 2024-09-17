use clap::{Parser, Subcommand};
use eyre::eyre;
use sha1::Digest;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

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
    HashObject {
        #[clap(short = 'w', long = "write")]
        path: PathBuf,
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
            dbg!(compressed.len());
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
        Command::HashObject { path } => {
            // Create the object input
            let content = fs::read(path)?;
            let header = format!("blob {}\0", content.len());
            let object = [header.as_bytes(), &content].concat();

            // Compute the hash of the object
            let mut hasher = sha1::Sha1::new();
            hasher.update(&object);
            let hash = hasher.finalize();
            let hash = hex::encode(hash.as_slice());

            // Compress the object
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Default::default());
            encoder.write_all(&object)?;
            let compressed = encoder.finish()?;

            // Write the compressed data to output
            let base_path = format!(".git/objects/{}", &hash[..2]);
            let output_path = format!("{}/{}", base_path, &hash[2..]);
            let _ = fs::create_dir(base_path);
            fs::write(output_path, compressed)?;

            print!("{}", hash);
            Ok(())
        }
    }
}
