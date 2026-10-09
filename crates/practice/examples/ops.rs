#![forbid(unsafe_code)]
//! Standalone, local-only project operations; credentials and user data stay untouched.
#[cfg(unix)]
#[path = "ops/gui_tests.rs"]
mod gui_tests;
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
        println!("./ops <check|build|local:deploy|companion|agents|snapshot|cache:fetch|contests:refresh|workspace:move|release:package|trace|gui:test|toolchain:setup|snippets> [--json]\n\ncompanion     Login browser-import listener; use ./ops companion --help.\n\nagents        Detect local agents; --catalog lists live models, --smoke runs read-only JSON probes.\nsnippets      Inspect or change local snippets; use ./ops snippets --help.\ncheck         Rust workspace tests.\nbuild         Release desktop build.\nlocal:deploy  Build/install leet and 1337 commands, desktop entry/icon; verify version and links.\nsnapshot      Public-only SQLite refresh; use ./ops snapshot --help.\ncache:fetch   Cache one Codeforces statement: --slug cf:CONTEST:INDEX.\ncontests:refresh  Refresh both providers; optional --contest ID or --leetcode-contest SLUG.\nworkspace:move Move solutions and Git history: --path /absolute/path; preserve a compatibility link.\nrelease:package  Package a native CI build; use ./ops release:package --help.\ngui:test      Isolated GPUI interaction, PNG and optional video checks; use ./ops gui:test --help.\ntrace         Record one case; use ./ops trace --help.\n\nLocal environment. Preserves settings, credentials, cache, and Solutions. Running windows offer restart.");return Ok(());
    }
    if command == "companion" {
        let status = Command::new("cargo").args(["run", "--quiet", "-p", "practice", "--example", "companion_service", "--"]).args(&args[1..]).status()?;
        if !status.success() { bail!("Companion service command failed"); }
        return Ok(());
    }
    if command == "snippets" { return practice::snippets::cli::cli(&args[1..]); }
    if command == "gui:test" { return super::gui_tests::run(&args[1..]); }
    if command == "trace" { return trace(&args[1..]); }
    if command == "agents" {
        if args.iter().skip(1).any(|arg| !matches!(arg.as_str(), "--json"|"--catalog"|"--smoke"|"--help")) { bail!("Use ./ops agents [--json] [--catalog] [--smoke]"); }
        if args.iter().any(|arg| arg == "--help") { println!("./ops agents [--json] [--catalog] [--smoke]\nLocal installed CLIs. Catalog discovery is read-only; smoke sends a small read-only prompt using each default model."); return Ok(()); }
        let catalogs = args.iter().any(|arg| matches!(arg.as_str(), "--catalog"|"--smoke"));
        let smoke = args.iter().any(|arg| arg == "--smoke");
        let mut records = Vec::new(); let mut failed = false;
        for agent in practice::agents::detect() {
            let mut record = json!({"kind":agent.kind,"path":agent.path,"version":agent.version});
            if catalogs { match practice::agents::catalog(&agent) {
                Ok(catalog) => {
                    if catalog.models.is_empty() { failed=true; record["catalog_error"]=json!(catalog.sign_in_hint.as_deref().unwrap_or("Agent returned no models")); }
                    if smoke { if let Some(model) = catalog.default_model.clone() {
                        let model = if agent.kind == practice::agents::AgentKind::Antigravity { catalog.models.iter().find(|entry| entry.id.starts_with(model.trim_end_matches("-high").trim_end_matches("-medium")) && entry.id.ends_with("-low")).map(|entry|entry.id.clone()).unwrap_or(model) } else {model};
                        let effort = catalog.models.iter().find(|entry| entry.id == model).and_then(|entry| entry.efforts.first()).map(|effort| effort.id.clone());
                        let request = practice::agents::Request { agent:agent.clone(), selection:practice::agents::Selection { agent:agent.kind, model, effort, fast:false }, prompt:"Return exactly {\"answer\":\"pong\"}. Answer immediately from this prompt. Do not use tools or create planning artifacts or walkthroughs.".into(), schema:Some(json!({"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"],"additionalProperties":false})), cwd:std::env::temp_dir(), access:practice::agents::Access::ReadOnly, resume:None };
                        let cancel = practice::agents::Cancel::default(); let deadline = cancel.clone();
                        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
                        let watchdog = std::thread::spawn(move || { if stop_rx.recv_timeout(std::time::Duration::from_secs(90)).is_err() { deadline.cancel(); } });
                        let result = practice::agents::run(&request, &mut |_| {}, &cancel);
                        let _ = stop_tx.send(()); let _ = watchdog.join();
                        match result { Ok(outcome) => { let passed = outcome.structured.as_ref().is_some_and(|value| value["answer"] == "pong"); failed |= !passed; record["smoke"] = json!({"passed":passed,"text":outcome.text,"structured":outcome.structured,"session":outcome.session,"model":request.selection.model}); }, Err(error) => { failed=true; record["smoke_error"]=json!(error.to_string()); } }
                    } else { failed=true; record["smoke_error"]=json!("Agent did not advertise a default model"); } }
                    record["catalog"]=serde_json::to_value(catalog)?;
                }
                Err(error) => { failed=true; record["catalog_error"]=json!(error.to_string()); }
            } }
            records.push(record);
        }
        println!("{}",json!({"command":"agents","environment":"local-agent-clis","agents":records,"success":!failed}));
        if failed { bail!("Some agent probes failed; see per-agent results"); } return Ok(());
    }
    if command == "workspace:move" {
        if args.len() < 3 || args.len() > 4 || args[1] != "--path" || args.get(3).is_some_and(|arg| arg != "--json") { bail!("Use ./ops workspace:move --path /absolute/path [--json]"); }
        let destination = PathBuf::from(&args[2]);
        if !destination.is_absolute() { bail!("Workspace path must be absolute"); }
        let mut config = practice::config::Config::load()?;
        let source = config.workspace.clone();
        let moved = if source == destination { config.save()?; false } else {
            if destination.symlink_metadata().is_ok() { bail!("Destination already exists; refusing to overwrite solutions"); }
            if destination.starts_with(&source) || source.symlink_metadata()?.file_type().is_symlink() || !source.is_dir() { bail!("Workspace must be a directory and destination must be outside it"); }
            std::fs::rename(&source, &destination).context("Move workspace on the same filesystem")?;
            if let Err(error) = std::os::unix::fs::symlink(&destination, &source) {
                std::fs::rename(&destination, &source)?; return Err(error.into());
            }
            config.workspace = destination.clone();
            if let Err(error) = config.save() {
                std::fs::remove_file(&source)?; std::fs::rename(&destination, &source)?; return Err(error);
            }
            true
        };
        println!("{}", json!({"command":command,"environment":"local-user-workspace","source":source,"destination":destination,"moved":moved,"compatibility_link":moved,"config":practice::config::Config::path()}));
        return Ok(());
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
        if args.iter().skip(1).any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
            println!("./ops contests:refresh [--contest ID | --leetcode-contest SLUG] [--json]\nRefresh public Codeforces and LeetCode lists in the local user cache. Optional contest selection also caches its problems. Reports partial failures and preserves previous provider data.");
            return Ok(());
        }
        let mut id = None; let mut leetcode = None; let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--json" => index += 1,
                "--contest" if id.is_none() && leetcode.is_none() => {
                    index += 1; let parsed: u32 = args.get(index).context("--contest requires an ID")?.parse()?;
                    if parsed == 0 { bail!("Contest ID must be positive"); } id = Some(parsed); index += 1;
                }
                "--leetcode-contest" if leetcode.is_none() && id.is_none() => {
                    index += 1; let slug = args.get(index).context("--leetcode-contest requires a slug")?;
                    if !practice::contests::valid_leetcode_slug(slug) { bail!("Invalid LeetCode contest slug"); }
                    leetcode = Some(slug.clone()); index += 1;
                }
                _ => bail!("Use ./ops contests:refresh [--contest ID | --leetcode-contest SLUG] [--json]"),
            }
        }
        let path = practice::config::database_path(); let db = practice::db::Db::open(&path)?;
        let (entries, errors) = practice::contests::refresh_entries(&db)?;
        let contests: Vec<_> = entries.iter().filter_map(|entry| match entry { practice::contests::ContestEntry::Codeforces(contest) => Some(contest), _ => None }).collect();
        let leetcode_contests: Vec<_> = entries.iter().filter_map(|entry| match entry { practice::contests::ContestEntry::LeetCode(contest) => Some(contest), _ => None }).collect();
        let leetcode_problems = leetcode.as_ref().map(|slug| practice::contests::refresh_contest_problems(&db, &practice::contests::ContestId::LeetCode(slug.clone()), &practice::leetcode::Client::new(None))).transpose()?;
        let leetcode_problems = match leetcode_problems { Some(practice::contests::ContestProblems::LeetCode(items)) => Some(items), _ => None };
        let problems = id.map(|id| practice::contests::refresh_problems(&db, id)).transpose()?;
        println!("{}", json!({"command":command,"environment":"local-user-cache","database":path,"contests":contests.len(),"upcoming":contests.iter().filter(|contest| !contest.past()).take(3).collect::<Vec<_>>(),"leetcode_contests":leetcode_contests,"leetcode_contest":leetcode,"leetcode_problems":leetcode_problems,"errors":errors,"success":errors.is_empty(),"contest_id":id,"problems":problems.map(|problems|problems.len())}));
        if !errors.is_empty() { bail!("Some contest providers failed to refresh; cached lists were preserved"); }
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
fn trace(args:&[String])->Result<()> {
    const USAGE:&str="./ops trace --language python|cpp --solution PATH (--stdin | --meta PATH) --input PATH [--json] [--max-steps N]";
    if args.iter().any(|arg|matches!(arg.as_str(),"--help"|"-h")){println!("{USAGE}\nLocal solution execution; no account, cache, or solution repository writes.");return Ok(());}
    let mut language=None;let mut solution=None;let mut meta=None;let mut input=None;let mut max_steps=4000;let mut json_output=false;let mut stdin=false;let mut i=0;
    while i<args.len(){
        let option=args[i].as_str();
        if option=="--json" {json_output=true;i+=1;continue;}
        if option=="--stdin" && !stdin {stdin=true;i+=1;continue;}
        let value=args.get(i+1).with_context(||format!("{option} requires a value; {USAGE}"))?;
        match option {
            "--language" if language.is_none()=>language=Some(match value.as_str(){"python"=>practice::language::Language::Python,"cpp"=>practice::language::Language::Cpp,_=>bail!("Language must be python or cpp")}),
            "--solution" if solution.is_none()=>solution=Some(PathBuf::from(value)),
            "--meta" if meta.is_none()=>meta=Some(PathBuf::from(value)),
            "--input" if input.is_none()=>input=Some(PathBuf::from(value)),
            "--max-steps"=>max_steps=value.parse().context("max-steps must be a nonnegative integer")?,
            _=>bail!("Unexpected option {option}; {USAGE}"),
        }i+=2;
    }
    let language=language.context(USAGE)?;let solution=solution.context(USAGE)?;
    if stdin && meta.is_some() { bail!("Choose --stdin or --meta, not both"); }
    let meta:serde_json::Value=if stdin { serde_json::Value::Null } else { serde_json::from_str(&std::fs::read_to_string(meta.context(USAGE)?)?)? };
    let case=practice::runner::Case{id:0,input:std::fs::read_to_string(input.context(USAGE)?)?,expected:None,custom:true};
    let started=Instant::now();
    let limits=practice::debugger::Limits{max_steps,..Default::default()};
    let trace=if stdin { practice::debugger::record_stdin(language,"python3",&solution,&case,limits)? } else { practice::debugger::record(language,"python3",&solution,&meta,&case,limits)? };
    let ms=started.elapsed().as_secs_f64()*1000.;
    if json_output{println!("{}",json!({"command":"trace","environment":"local","steps_count":trace.steps.len(),"ms":ms,"trace":trace}));}
    else{println!("{} steps, truncated={}, {:.1} ms\noutput: {}",trace.steps.len(),trace.truncated,ms,trace.output.as_deref().unwrap_or("(none)"));if let Some(error)=trace.error{println!("error: {error}");}}
    Ok(())
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
    let entry=format!("[Desktop Entry]\nType=Application\nName=leet\nGenericName=Coding practice IDE\nComment=NeetCode, LeetCode, Codeforces and CodeChef practice\nExec=\"{exec}\"\nIcon=leet\nTerminal=false\nStartupWMClass=leet\nCategories=Development;\nKeywords=leetcode;neetcode;codeforces;codechef;1337;\n");
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
fn main() {
    if std::env::args().nth(1).as_deref() == Some("toolchain:setup") {
        if let Err(error) = practice::tool_setup::cli(&std::env::args().skip(2).collect::<Vec<_>>()) { eprintln!("{error:#}"); std::process::exit(1); }
    } else { unix::main(); }
}

#[cfg(not(unix))]
fn main() {
    if std::env::args().nth(1).as_deref() == Some("toolchain:setup") {
        if let Err(error) = practice::tool_setup::cli(&std::env::args().skip(2).collect::<Vec<_>>()) { eprintln!("{error:#}"); std::process::exit(1); }
        return;
    }
    eprintln!("Local desktop deployment uses Unix. For Windows releases use cargo run -p practice --example release -- --help.");
    std::process::exit(1);
}
