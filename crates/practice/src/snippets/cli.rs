//! Validated snippet tools available to every installed coding agent.
use std::{io::Read, path::Path};
use anyhow::{Result, bail, ensure};
use serde_json::json;
use super::{Snippet, Origin, store};

pub fn context() -> String {
    let executable = std::env::current_exe().unwrap_or_else(|_| "leet".into());
    format!("\nLeet has editable competitive-programming snippets for Python, C++, Go, C, and Java. Tool command: {:?} snippets. Use `list`, `show --key SCOPE/NAME`, `set` (Snippet JSON on stdin), and `remove --key SCOPE/NAME`. All return JSON. `set --preview` validates without saving. Never edit snippet files directly. Ask the user's language, style, trigger, and cursor preferences before generating; use ACP elicitation/create if available, otherwise ask in chat and wait for their answer. Only configure snippets when requested. Templates use template=true; bodies support $0, numbered placeholders, choices, mirrors, and regex transforms.\n", executable)
}

pub fn cli(args: &[String]) -> Result<()> {
    if args.is_empty() || args.iter().any(|s| s == "--help") {
        println!("leet snippets list|show|set|remove [--key SCOPE/NAME] [--preview]\nset reads Snippet JSON from stdin; preview validates without saving. Results are JSON.");
        return Ok(());
    }
    let dir = args.windows(2).find(|p|p[0]=="--directory").map_or_else(store::dir,|p|std::path::PathBuf::from(&p[1]));
    let config = crate::config::Config::load()?;
    let result = execute(&dir, &config.snippets, args, &mut std::io::stdin())?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

pub fn execute(dir: &Path, settings: &super::Settings, args: &[String], input: &mut impl Read) -> Result<serde_json::Value> {
    let loaded = store::load(dir);
    let effective = store::effective(&loaded.snippets, &super::builtin(), settings);
    let key = args.windows(2).find(|pair| pair[0] == "--key").map(|pair| pair[1].as_str());
    let expected = loaded.snippets.clone();
    let mut user = loaded.snippets;
    match args.first().map(String::as_str) {
        Some("list") => Ok(json!({"snippets": effective, "errors": loaded.errors})),
        Some("show") => Ok(serde_json::to_value(effective.iter().find(|s| Some(s.key().as_str()) == key).ok_or_else(|| anyhow::anyhow!("Snippet not found"))?)?),
        Some("set") => {
            ensure!(loaded.errors.is_empty(), "Repair invalid snippet files before saving: {:?}", loaded.errors);
            let mut bytes = Vec::new(); std::io::Read::take(input,300_001).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 300_000, "Snippet JSON exceeds 300 KiB");
            let mut snippet: Snippet = serde_json::from_slice(&bytes)?;
            snippet.origin = Origin::Ai;
            store::check(&snippet, &[]).map_err(anyhow::Error::msg)?;
            user.retain(|s| s.key() != snippet.key()); user.push(snippet.clone());
            if !args.iter().any(|s| s == "--preview") { store::commit(dir, &expected, &user)?; }
            Ok(json!({"snippet": snippet, "saved": !args.iter().any(|s| s == "--preview"), "preview": super::body::preview(&snippet.body).text}))
        }
        Some("remove") => {
            ensure!(loaded.errors.is_empty(), "Repair invalid snippet files before saving");
            let key = key.ok_or_else(|| anyhow::anyhow!("Supply --key SCOPE/NAME"))?;
            let before = user.len(); user.retain(|s| s.key() != key);
            ensure!(user.len() != before, "User snippet not found; hide built-ins in the snippet editor");
            if !args.iter().any(|s| s == "--preview") { store::commit(dir, &expected, &user)?; }
            Ok(json!({"removed":key,"saved":!args.iter().any(|s| s == "--preview")}))
        }
        _ => bail!("Unknown snippet command; use --help"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validated_cli_preview_and_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let s = Snippet { name:"Loop".into(), prefixes:vec!["loop".into()], body:"${1:i} $1 $0".into(), description:String::new(), scope:Some(crate::language::Language::Cpp), template:false, origin:Origin::User };
        let bytes = serde_json::to_vec(&s).unwrap();
        let settings = super::super::Settings::default();
        execute(dir.path(), &settings, &["set".into(), "--preview".into()], &mut bytes.as_slice()).unwrap();
        assert!(store::load(dir.path()).snippets.is_empty());
        execute(dir.path(), &settings, &["set".into()], &mut bytes.as_slice()).unwrap();
        assert_eq!(store::load(dir.path()).snippets.len(),1);
        std::fs::write(dir.path().join("java.json"), "broken").unwrap();
        assert!(execute(dir.path(), &settings, &["set".into()], &mut bytes.as_slice()).is_err());
    }
}
