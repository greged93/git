use eyre::eyre;
use sha1::Digest;
use std::fmt::Formatter;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

/// A file in the git file system.
#[derive(Debug)]
pub struct GitFile {
    pub(crate) file_content: GitFileContent,
    pub(crate) sha: Vec<u8>,
}

/// The content of a tree for a git file.
#[derive(Debug, Clone)]
pub struct TreeContent {
    mode: u32,
    name: String,
    sha: Vec<u8>,
}

impl std::fmt::Display for GitFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.file_content {
            GitFileContent::Blob(v) => {
                f.write_str(std::str::from_utf8(v).map_err(|_| std::fmt::Error)?)
            }
            GitFileContent::Tree(t) => {
                t.iter().for_each(|t| {
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

        // Read the content of the git file.
        // For a tree, we split the input into a [`TreeContent`] structure.
        let content = if header.contains("tree") {
            let mut tree_content = Vec::new();
            while let (Some(zero_byte), Some(space)) = (
                content.iter().position(|x| x == &b'\0'),
                content.iter().position(|x| x == &b' '),
            ) {
                // Tree files are split into MODE NAME\0SHA-1
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

        let content = GitFileContent::Blob(git_file_content);

        Ok(Self {
            file_content: content,
            sha: sha.to_vec(),
        })
    }

    /// Returns a [`GitFile`] with a content corresponding to the created tree
    pub fn from_directory(path: PathBuf) -> eyre::Result<Self> {
        if !path.is_dir() {
            return Err(eyre!("expected dir path"));
        }

        let files = std::fs::read_dir(&path)?;

        let items = files
            .filter_map(|e| {
                let entry = e.ok()?;
                let name = entry
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Ignore the directory itself
                if entry.path() == path {
                    return None;
                }
                // Ignore the .git directory
                if entry.path().iter().any(|dir| dir.to_str() == Some(".git")) {
                    return None;
                }

                if entry.path().is_dir() {
                    let tree = Self::from_directory(entry.path()).ok()?;
                    let sha = tree.sha;
                    let mode = 40000;
                    Some(TreeContent { mode, sha, name })
                } else {
                    let blob = Self::from_file(entry.path()).ok()?;
                    let sha = blob.sha;
                    let mode = 100644;
                    Some(TreeContent { mode, sha, name })
                }
            })
            .collect();

        let content = GitFileContent::Tree(items);
        let c = content.content();

        // Hash the git file
        let mut hasher = sha1::Sha1::new();
        hasher.update(c);
        let sha = hasher.finalize();

        Ok(Self {
            file_content: content,
            sha: sha.to_vec(),
        })
    }

    /// Returns the compressed content of the file.
    pub fn compress(&self) -> eyre::Result<Vec<u8>> {
        // Compress the object
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Default::default());

        encoder.write_all(&self.content())?;
        Ok(encoder.finish()?)
    }

    /// Returns the sha-1 hash of the file.
    pub fn hash(&self) -> &[u8] {
        &self.sha
    }

    /// Returns the raw content of the file.
    pub fn content(&self) -> Vec<u8> {
        self.file_content.content()
    }
}

/// The content of a git file.
#[derive(Debug)]
pub enum GitFileContent {
    Blob(Vec<u8>),
    Tree(Vec<TreeContent>),
    Commit,
}

impl GitFileContent {
    /// Returns the raw content of the file.
    pub fn content(&self) -> Vec<u8> {
        match &self {
            GitFileContent::Blob(c) => c.clone(),
            GitFileContent::Tree(trees) => {
                // Tree files are split into MODE NAME\0SHA-1
                let mut trees = trees.clone();
                trees.sort_by(|a, b| a.name.cmp(&b.name));
                let content = trees
                    .into_iter()
                    .flat_map(|t| {
                        let s = format!("{} {}\0", t.mode, t.name);
                        [s.as_bytes(), &t.sha].concat()
                    })
                    .collect::<Vec<_>>();
                let header = format!("tree {}\0", content.len());
                [header.as_bytes(), &content].concat()
            }
            _ => vec![],
        }
    }
}
