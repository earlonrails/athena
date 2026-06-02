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
            if config.cron_jobs.is_empty() {
                outro("No active Athena cron jobs found.")?;
            } else {
                let mut msg = String::from("Active Jobs:\n");
                for (i, job) in config.cron_jobs.iter().enumerate() {
                    let chan_str = job.channel.map(|c| format!(" (Channel: {})", c)).unwrap_or_default();
                    let thread_str = job.thread.map(|t| format!(" (Thread: {})", t)).unwrap_or_default();
                    msg.push_str(&format!("  [{}] '{}' -> {}{}{}\n", i, job.schedule, job.query, chan_str, thread_str));
                }
                outro(msg.trim_end())?;
            }
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
                channel = chan_str.parse().ok();
                
                let add_thread: bool = confirm("Would you like to route this to a specific thread inside the channel?")
                    .initial_value(false)
                    .interact()?;
                    
                if add_thread {
                    let thread_str: String = input("Enter Thread ID (e.g. 1234)")
                        .interact()?;
                    thread = thread_str.parse().ok();
                }
            }

            config.cron_jobs.push(CronJob {
                schedule: schedule.clone(),
                query: query.clone(),
                channel,
                thread,
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

// Rust guideline compliant 2026-02-21
