//! Editable native response files. Parsing never evaluates scripts or HTML.
use std::io::Read as _;
use std::path::{Path, PathBuf};
use anyhow::{Result, bail};
use crate::assist::{Answer, Reply};

pub fn directory(message: i64) -> PathBuf { crate::config::data_dir().join("chat-artifacts").join(message.to_string()) }
pub fn path(message: i64) -> PathBuf { directory(message).join("response.json") }
pub fn load(path: &Path) -> Result<Answer> {
    let mut data = Vec::new();
    std::fs::File::open(path)?.take(2_000_001).read_to_end(&mut data)?;
    if data.len() > 2_000_000 { bail!("Native response artifact exceeds 2 MB"); }
    Ok(serde_json::from_slice::<Reply>(&data)?.response)
}
pub fn save(path: &Path, answer: &Answer) -> Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, serde_json::to_vec_pretty(&Reply { response: answer.clone() })?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artifacts_can_be_edited_and_invalid_content_is_rejected() {
        let dir = tempfile::tempdir().unwrap(); let file = dir.path().join("response.json");
        save(&file, &Answer::Chat("first".into())).unwrap();
        assert_eq!(load(&file).unwrap(), Answer::Chat("first".into()));
        std::fs::write(&file, r#"{"response":{"action":"chat","answer":"edited"}}"#).unwrap();
        assert_eq!(load(&file).unwrap(), Answer::Chat("edited".into()));
        std::fs::write(&file, "<script>run()</script>").unwrap(); assert!(load(&file).is_err());
    }
}
