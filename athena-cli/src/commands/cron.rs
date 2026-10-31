use cliclack::{intro, select, input, confirm, outro, outro_cancel};
use anyhow::Result;
use athena_core::config::{load_config, save_config, CronJob};

pub fn run_cron() -> Result<()> {
    intro("Athena Cron Job Scheduler")?;

    let choice: usize = select("Manage periodic background agent queries via the Gateway")
        .item(1, "List active Athena cron jobs", "")
        .item(2, "Add a new scheduled query", "")
        .item(3, "Remove a specific Athena cron job", "")
        .item(4, "Remove all Athena cron jobs", "")
        .item(5, "Exit", "")
        .interact()?;

    let mut config = load_config();

    match choice {
        1 => {
            println!("\nActive internal cron jobs for Athena:");
            outro(format_cron_jobs(&config.cron_jobs))?;
        }
        2 => {
            let schedule: String = input("Enter cron schedule")
                .placeholder("0 * * * *")
                .interact()?;

            if schedule.is_empty() {
                outro_cancel("Schedule cannot be empty.")?;
                return Ok(());
            }

            let query: String = input("Enter the query for the agent")
                .placeholder("check server health")
                .interact()?;

            if query.is_empty() {
                outro_cancel("Query cannot be empty.")?;
                return Ok(());
            }
            
            let mut channel = None;
            let mut thread = None;
            
            let add_channel: bool = confirm("Would you like to route this cron job to a specific Telegram Channel/Chat?")
                .initial_value(false)
                .interact()?;
                
            if add_channel {
                let chan_str: String = input("Enter Telegram Channel/Chat ID (e.g. -100123456789)")
                    .interact()?;
                channel = Some(chan_str);
                
                let add_thread: bool = confirm("Would you like to route this to a specific thread inside the channel?")
                    .initial_value(false)
                    .interact()?;
                    
                if add_thread {
                    let thread_str: String = input("Enter Thread ID (e.g. 1234)")
                        .interact()?;
                    thread = Some(thread_str);
                }
            }

            config.cron_jobs.push(CronJob {
                schedule: schedule.clone(),
                query: query.clone(),
                channel,
                thread,
                delivery: Some(vec!["telegram".to_string()]),
            });

            if save_config(&config).is_ok() {
                outro(format!("Successfully added scheduled job!\nJob: {} -> {}", schedule, query))?;
            } else {
                outro_cancel("Failed to save configuration.")?;
            }
        }
        3 => {
            if config.cron_jobs.is_empty() {
                outro("No active Athena cron jobs found to remove.")?;
                return Ok(());
            }
            
            let mut items = Vec::new();
            for (i, job) in config.cron_jobs.iter().enumerate() {
                items.push((i, format!("{} -> {}", job.schedule, job.query)));
            }
            
            let mut prompt = select("Select the cron job to remove");
            for (i, desc) in items {
                prompt = prompt.item(i, desc, "");
            }
            let idx_to_remove: usize = prompt.interact()?;
            
            let removed = config.cron_jobs.remove(idx_to_remove);
            
            if save_config(&config).is_ok() {
                outro(format!("Successfully removed cron job:\n{} -> {}", removed.schedule, removed.query))?;
            } else {
                outro_cancel("Failed to save configuration.")?;
            }
        }
        4 => {
            let confirm_rm: bool = confirm("Are you sure you want to remove ALL scheduled Athena cron jobs?")
                .interact()?;

            if !confirm_rm {
                outro_cancel("Cancelled.")?;
                return Ok(());
            }

            config.cron_jobs.clear();

            if save_config(&config).is_ok() {
                outro("All Athena cron jobs have been removed.")?;
            } else {
                outro_cancel("Failed to clear cron configuration.")?;
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

pub fn format_cron_jobs(jobs: &[CronJob]) -> String {
    if jobs.is_empty() {
        return "No active Athena cron jobs found.".to_string();
    }
    let mut msg = String::from("Active Jobs:\n");
    for (i, job) in jobs.iter().enumerate() {
        let chan_str = job.channel.clone().map(|c| format!(" (Channel: {})", c)).unwrap_or_default();
        let thread_str = job.thread.clone().map(|t| format!(" (Thread: {})", t)).unwrap_or_default();
        msg.push_str(&format!("  [{}] '{}' -> {}{}{}\n", i, job.schedule, job.query, chan_str, thread_str));
    }
    msg.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cron_jobs_empty() {
        let empty: Vec<CronJob> = vec![];
        assert_eq!(format_cron_jobs(&empty), "No active Athena cron jobs found.");
    }

    #[test]
    fn test_format_cron_jobs_populated() {
        let jobs = vec![
            CronJob {
                schedule: "0 9 * * *".into(),
                query: "Check stock prices".into(),
                channel: Some("123456".into()),
                thread: Some("99".into()),
                delivery: Some(vec!["telegram".into()]),
            },
            CronJob {
                schedule: "*/5 * * * *".into(),
                query: "Ping healthcheck".into(),
                channel: None,
                thread: None,
                delivery: None,
            },
        ];

        let res = format_cron_jobs(&jobs);
        assert!(res.contains("Active Jobs:"));
        assert!(res.contains("[0] '0 9 * * *' -> Check stock prices (Channel: 123456) (Thread: 99)"));
        assert!(res.contains("[1] '*/5 * * * *' -> Ping healthcheck"));
    }
}

// Rust guideline compliant 2026-02-21
