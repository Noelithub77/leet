#![forbid(unsafe_code)]
//! Standalone, local-only project operations; credentials and user data stay untouched.
#[cfg(unix)]
mod unix {
use std::{path::{Path,PathBuf},process::{Command,Stdio},time::{Instant,SystemTime,UNIX_EPOCH}};
use anyhow::{Result,Context,bail};
use serde_json::json;

fn run(program:&str,args:&[&str])->Result<()> {
    let output=Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::inherit()).output()?;
    eprint!("{}",String::from_utf8_lossy(&output.stdout));
    if !output.status.success(){bail!("{program} failed with {}",output.status);} Ok(())
}
pub fn main(){if let Err(error)=execute(){eprintln!("{}",json!({"environment":"local","error":error.to_string()}));std::process::exit(1);}}
fn execute()->Result<()> {
    let args:Vec<_>=std::env::args().skip(1).collect();
    let command=args.first().map(String::as_str).unwrap_or("--help");
    if matches!(command,"--help"|"-h"|"help"){
        println!("./ops <check|build|local:deploy|snapshot|cache:fetch|contests:refresh|release:package> [--json]\n\ncheck         Rust workspace tests.\nbuild         Release desktop build.\nlocal:deploy  Build/install leet and 1337 commands, desktop entry/icon; verify version and links.\nsnapshot      Public-only SQLite refresh; use ./ops snapshot --help.\ncache:fetch   Cache one Codeforces statement: --slug cf:CONTEST:INDEX.\ncontests:refresh  Refresh cached Codeforces contests; optional --contest ID.\nrelease:package  Package a native CI build; use ./ops release:package --help.\n\nLocal environment. Preserves settings, credentials, cache, and Solutions. Running windows offer restart.");return Ok(());
    }
    if command == "cache:fetch" {
        if args.len() < 3 || args.len() > 4 || args[1] != "--slug" || args.get(3).is_some_and(|arg| arg != "--json") {
            bail!("Use ./ops cache:fetch --slug cf:CONTEST:INDEX [--json]");
        }
        practice::codeforces::problem_id(&args[2])?;
        let path = practice::config::database_path(); let db = practice::db::Db::open(&path)?;
        let cached = db.question(&args[2])?.is_some();
        let q = practice::codeforces::cached_question(&db, &args[2])?;
        println!("{}", json!({"command":command,"environment":"local-user-cache","database":path,"slug":q.slug,"cache_hit":cached,"statement_source":q.meta["statementSource"],"samples":q.examples.len(),"statement_bytes":q.content.len()}));
        return Ok(());
    }
    if command == "contests:refresh" {
        let mut id = None; let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--json" => index += 1,
                "--contest" if id.is_none() => {
                    index += 1; let parsed: u32 = args.get(index).context("--contest requires an ID")?.parse()?;
                    if parsed == 0 { bail!("Contest ID must be positive"); } id = Some(parsed); index += 1;
                }
                _ => bail!("Use ./ops contests:refresh [--contest ID] [--json]"),
            }
        }
        let path = practice::config::database_path(); let db = practice::db::Db::open(&path)?;
        let contests = practice::contests::refresh_list(&db)?;
        let problems = id.map(|id| practice::contests::refresh_problems(&db, id)).transpose()?;
        println!("{}", json!({"command":command,"environment":"local-user-cache","database":path,"contests":contests.len(),"upcoming":contests.iter().filter(|contest| !contest.past()).take(3).collect::<Vec<_>>(),"contest_id":id,"problems":problems.map(|problems|problems.len())}));
        return Ok(());
    }
    if args.iter().skip(1).any(|arg|arg!="--json"){bail!("Unexpected argument; use ./ops --help");}
    match command {
        "check"=>{run("cargo",&["test","--workspace"])?;run("python3",&["tests/install.py"])?;},
        "build"=>run("nice",&["-n","10","cargo","build","--release","-p","gui"])? ,
        "local:deploy"=>{
            let started=Instant::now();
            let mut stages=Vec::new();
            let result=(|| -> Result<_> {
            let stage=Instant::now();
            run("nice",&["-n","10","cargo","build","--release","-p","gui"])?;
            let build_duration=stage.elapsed();
            if build_duration >= std::time::Duration::from_secs(1) {
                stages.push(json!({"name":"build","duration_seconds":build_duration.as_secs_f64()}));
            }
            let home=dirs::home_dir().context("Home directory unavailable")?;
            let bin=home.join(".local/bin");let builds=home.join(".local/share/leet/bin");
            let stage=Instant::now();
            std::fs::create_dir_all(&bin)?;std::fs::create_dir_all(&builds)?;
            let revision=Command::new("git").args(["rev-parse","--short","HEAD"]).output()?;
            if !revision.status.success(){bail!("Commit the development checkpoint before installing");}
            let revision=String::from_utf8(revision.stdout)?.trim().to_owned();
            let stamp=SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let installed=builds.join(format!("leet-{stamp}-{revision}"));
            std::fs::copy("target/release/leet",&installed)?;
            for name in ["leet", "1337"] { link(&installed, &bin.join(name))?; }
            stages.push(json!({"name":"install_binary_and_links","duration_seconds":stage.elapsed().as_secs_f64()}));
            let stage=Instant::now();
            let legacy = bin.join("vg");
            if legacy.symlink_metadata().is_ok() {
                let target = std::fs::canonicalize(&legacy)?;
                if !target.starts_with(&builds) { bail!("vg points to an unrelated executable; it was preserved"); }
                std::fs::remove_file(&legacy)?;
            }
            desktop(&home,&bin.join("leet"))?;
            stages.push(json!({"name":"desktop_entry","duration_seconds":stage.elapsed().as_secs_f64()}));
            let stage=Instant::now();
            for name in ["leet","1337"]{if std::fs::canonicalize(bin.join(name))?!=installed{bail!("{name} command verification failed");}}
            let version=Command::new(bin.join("leet")).arg("--version").output()?;
            if !version.status.success(){bail!("Installed leet could not report its version");}
            let version=String::from_utf8(version.stdout)?.trim().to_owned();
            stages.push(json!({"name":"verify_install","duration_seconds":stage.elapsed().as_secs_f64()}));
            let stage=Instant::now();
            prune(&builds,&installed)?;
            stages.push(json!({"name":"prune_old_builds","duration_seconds":stage.elapsed().as_secs_f64()}));
            Ok(json!({"command":command,"environment":"local","revision":revision,"binary":installed,"installed":bin.join("leet"),"removed_alias":legacy,"easter_egg_alias":bin.join("1337"),"desktop":home.join(".local/share/applications/leet.desktop"),"version":version}))
            })();
            let record=json!({
                "timestamp_unix":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                "duration_seconds":started.elapsed().as_secs_f64(),
                "stages":stages,
                "outcome":if result.is_ok(){"success"}else{"failure"},
                "error":result.as_ref().err().map(ToString::to_string)
            });
            append_deploy_timing(&record)?;
            println!("{}",json!({"command":command,"environment":"local","timings_file":"tests/local-deploy-timings.jsonl","timing":record,"result":result?}));return Ok(());
        },
        _=>bail!("Unknown command {command}; use ./ops --help"),
    }
    println!("{}",json!({"command":command,"environment":"local"}));Ok(())
}
fn append_deploy_timing(record:&serde_json::Value)->Result<()> {
    use std::io::Write;
    let path=Path::new("tests/local-deploy-timings.jsonl");
    let mut file=std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file,"{}",record)?;
    Ok(())
}
fn link(target:&Path,path:&Path)->Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|info|!info.file_type().is_symlink()){bail!("{} exists and is not a symlink; refusing to overwrite",path.display());}
    let temp=path.with_extension(format!("{}.tmp",std::process::id()));
    if std::fs::symlink_metadata(&temp).is_ok(){std::fs::remove_file(&temp)?;}
    std::os::unix::fs::symlink(target,&temp)?;std::fs::rename(temp,path)?;Ok(())
}
fn desktop(home:&Path,command:&Path)->Result<()> {
    let icons=home.join(".local/share/icons/hicolor/scalable/apps");let apps=home.join(".local/share/applications");
    std::fs::create_dir_all(&icons)?;std::fs::create_dir_all(&apps)?;
    std::fs::copy("crates/gui/assets/leet.svg",icons.join("leet.svg"))?;
    let exec=command.to_string_lossy().replace('\\',"\\\\").replace('"',"\\\"").replace('`',"\\`").replace('$',"\\$");
    let entry=format!("[Desktop Entry]\nType=Application\nName=leet\nGenericName=Coding practice IDE\nComment=NeetCode, LeetCode and Codeforces practice\nExec=\"{exec}\"\nIcon=leet\nTerminal=false\nStartupWMClass=leet\nCategories=Development;\nKeywords=leetcode;neetcode;codeforces;1337;\n");
    std::fs::write(apps.join("leet.desktop"),entry)?;
    // Retire only our old launcher; unrelated user desktop entries stay untouched.
    let old=apps.join("vg.desktop");
    if std::fs::read_to_string(&old).is_ok_and(|text|text.contains("Name=vg\n")&&text.contains("StartupWMClass=vg\n")) {std::fs::remove_file(old)?;}
    Ok(())
}
fn prune(dir:&Path,current:&Path)->Result<()> {
    let mut files:Vec<PathBuf>=std::fs::read_dir(dir)?.filter_map(|entry|entry.ok().map(|entry|entry.path())).filter(|path|path.file_name().is_some_and(|name|name.to_string_lossy().starts_with("leet-"))).collect();
    files.sort();let remove=files.len().saturating_sub(3);
    for path in files.into_iter().take(remove){if path!=current{std::fs::remove_file(path)?;}}Ok(())
}
}

#[cfg(unix)]
fn main() { unix::main(); }

#[cfg(not(unix))]
fn main() {
    eprintln!("Local desktop deployment uses Unix. For Windows releases use cargo run -p practice --example release -- --help.");
    std::process::exit(1);
}
