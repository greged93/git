use eyre::eyre;
use sha1::Digest;
use std::fmt::Formatter;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

/// A file in the git file system
#[derive(Debug)]
pub struct GitFile {
    header: String,
    file_content: GitFileContent,
    sha: Vec<u8>,
}

#[derive(Debug)]
pub enum GitFileContent {
    Blob(Vec<u8>),
    Tree(Vec<TreeContent>),
    Commit,
}

#[derive(Debug, Clone)]
pub struct TreeContent {
    mode: u32,
    name: String,
    sha: Vec<u8>,
}

#[derive(Debug)]
pub enum GitFileType {
    Blob,
    Tree,
    Commit,
}

impl std::fmt::Display for GitFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.file_content {
            GitFileContent::Blob(v) => {
                f.write_str(std::str::from_utf8(v).map_err(|_| std::fmt::Error)?)
            }
            GitFileContent::Tree(t) => {
                let mut c = t.clone();
                c.sort_by(|a, b| a.name.cmp(&b.name));
                c.iter().for_each(|t| {
                    let _ = f.write_str(&t.name);
                    let _ = writeln!(f);
                });
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl GitFile {
    /// Returns a [`GitFile`] with the content from the file located at
    /// `".git/objects/sha[..2]/sha[2..]"`.
    pub fn new(sha: String) -> eyre::Result<Self> {
        // Create the object input
        let path = format!(".git/objects/{}/{}", &sha[..2], &sha[2..]);
        let compressed = fs::read(path)?;

        // Decode the compressed file to a string
        let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
        let mut s = Vec::new();
        decoder.read_to_end(&mut s)?;

        // Hash the git file
        let mut hasher = sha1::Sha1::new();
        hasher.update(&s);
        let sha = hasher.finalize();

        // Split the header and the content
        let zero_byte_pos = &s
            .iter()
            .position(|x| x == &b'\0')
            .ok_or(eyre!("missing \0 byte"))?;
        let header = std::str::from_utf8(&s[..zero_byte_pos + 1])?.to_string();
        let mut content = &s[zero_byte_pos + 1..];

        let content = if header.contains("tree") {
            let mut tree_content = Vec::new();
            while let (Some(zero_byte), Some(space)) = (
                content.iter().position(|x| x == &b'\0'),
                content.iter().position(|x| x == &b' '),
            ) {
                let mode = std::str::from_utf8(&content[..space])?.parse::<u32>()?;
                let name = std::str::from_utf8(&content[space + 1..zero_byte])?.to_string();
                let sha = content[zero_byte + 1..zero_byte + 1 + 20].to_vec();
                let item = TreeContent { mode, name, sha };
                tree_content.push(item);
                content = &content[zero_byte + 1 + 20..];
            }
            GitFileContent::Tree(tree_content)
        } else if header.contains("commit") {
            GitFileContent::Commit
        } else {
            GitFileContent::Blob(content.to_vec())
        };

        Ok(Self {
            file_content: content,
            sha: sha.to_vec(),
            header,
        })
    }

    /// Returns a [`GitFile`] from the content of the file at the provided path.
    pub fn from_file(path: PathBuf) -> eyre::Result<Self> {
        let content = fs::read(path)?;
        let header = format!("blob {}\0", content.len());

        let git_file_content = [header.as_bytes(), content.as_slice()].concat();

        // Hash the git file
        let mut hasher = sha1::Sha1::new();
        hasher.update(&git_file_content);
        let sha = hasher.finalize();

        let content = GitFileContent::Blob(content);

        Ok(Self {
            file_content: content,
            header,
            sha: sha.to_vec(),
        })
    }

    /// Returns the compressed content of the file.
    pub fn compress(&self) -> eyre::Result<Vec<u8>> {
        // Compress the object
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Default::default());

        let content = match &self.file_content {
            GitFileContent::Blob(c) => c,
            _ => &vec![],
        };

        let content = [self.header.as_bytes(), content].concat();
        encoder.write_all(&content)?;
        Ok(encoder.finish()?)
    }

    /// Returns the sha-1 hash of the file.
    pub fn hash(&self) -> &[u8] {
        &self.sha
    }
}
