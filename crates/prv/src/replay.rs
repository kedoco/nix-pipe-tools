use crate::db::Database;
use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Debug)]
pub struct ReplayStep {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
}

pub fn plan_replay(db: &Database, target: &str) -> anyhow::Result<Vec<ReplayStep>> {
    // Build the full dependency tree by tracing backwards from the target
    let mut visited = HashSet::new();
    let mut steps: Vec<ReplayStep> = Vec::new();
    let mut queue = vec![target.to_string()];
    // Map from file -> (command, args, cwd, input files)
    let mut file_producers: HashMap<String, (String, Vec<String>, String, Vec<String>)> =
        HashMap::new();

    while let Some(file) = queue.pop() {
        if visited.contains(&file) {
            continue;
        }
        visited.insert(file.clone());

        let producers = db.producers_for_file(&file)?;
        if let Some((cmd, deps)) = producers.last() {
            let args: Vec<String> =
                serde_json::from_str(&cmd.args).unwrap_or_default();
            file_producers.insert(
                file.clone(),
                (cmd.command.clone(), args, cmd.cwd.clone(), deps.clone()),
            );
            for dep in deps {
                queue.push(dep.to_string());
            }
        }
    }

    // Topological sort: process files whose dependencies are already resolved
    let mut resolved = HashSet::new();
    let mut remaining: HashSet<String> = file_producers.keys().cloned().collect();

    while !remaining.is_empty() {
        let mut progress = false;
        let current: Vec<String> = remaining.iter().cloned().collect();
        for file in current {
            let (_, _, _, deps) = &file_producers[&file];
            let all_resolved = deps
                .iter()
                .all(|d| resolved.contains(d) || !file_producers.contains_key(d));
            if all_resolved {
                let (cmd, args, cwd, _) = &file_producers[&file];
                steps.push(ReplayStep {
                    command: cmd.clone(),
                    args: args.clone(),
                    cwd: cwd.clone(),
                });
                resolved.insert(file.clone());
                remaining.remove(&file);
                progress = true;
            }
        }
        if !progress {
            // Cycle detected, just add remaining in arbitrary order
            for file in &remaining {
                let (cmd, args, cwd, _) = &file_producers[file];
                steps.push(ReplayStep {
                    command: cmd.clone(),
                    args: args.clone(),
                    cwd: cwd.clone(),
                });
            }
            break;
        }
    }

    Ok(steps)
}

pub fn execute_replay(steps: &[ReplayStep], dry_run: bool) -> anyhow::Result<()> {
    for step in steps {
        let cmdline = if step.args.is_empty() {
            step.command.clone()
        } else {
            format!("{} {}", step.command, step.args.join(" "))
        };

        if dry_run {
            println!("[dry-run] cd {} && {}", step.cwd, cmdline);
            continue;
        }

        println!("$ cd {} && {}", step.cwd, cmdline);
        let status = Command::new(&step.command)
            .args(&step.args)
            .current_dir(&step.cwd)
            .status()?;

        if !status.success() {
            anyhow::bail!(
                "Command failed with exit code {}: {}",
                status.code().unwrap_or(-1),
                cmdline
            );
        }
    }
    Ok(())
}
