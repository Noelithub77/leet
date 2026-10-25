//! Normalize public provider pages without importing account or progress data.
use anyhow::{Result, bail, ensure};
use practice::striver::{self, Sheet};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("striver_snapshot --a2z-html PATH --sde-html PATH --output PATH\n\nReads downloaded public Prep Hub pages and writes public-only curriculum metadata.\nDoes not access accounts, personal progress, or remote mutations.");
        return Ok(());
    }
    let mut a2z = None;
    let mut sde = None;
    let mut output = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let target = match arg.as_str() { "--a2z-html" => &mut a2z, "--sde-html" => &mut sde, "--output" => &mut output, _ => bail!("Unknown argument: {arg}") };
        ensure!(target.is_none(), "Repeated argument: {arg}");
        *target = Some(iter.next().ok_or_else(|| anyhow::anyhow!("Missing value for {arg}"))?);
    }
    let mut curricula = Vec::new();
    for (sheet, path) in [(Sheet::A2z, a2z), (Sheet::Sde, sde)] {
        let path = path.ok_or_else(|| anyhow::anyhow!("Both public sheet pages are required"))?;
        curricula.push(striver::parse_page(&std::fs::read_to_string(path)?, sheet)?);
    }
    let output = output.ok_or_else(|| anyhow::anyhow!("--output is required"))?;
    std::fs::write(output, serde_json::to_vec_pretty(&curricula)?)?;
    println!("{}", serde_json::json!({"environment":"public-provider-metadata", "output":output, "sheets":curricula.iter().map(|c| serde_json::json!({"title":c.title,"sections":c.sections.len(),"items":c.sections.iter().map(|s|s.items.len()).sum::<usize>()})).collect::<Vec<_>>()}));
    Ok(())
}
