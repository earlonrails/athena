use athena_state::kanban::KanbanDB;
use cliclack::{intro, select, input, outro, outro_cancel};
use anyhow::Result;

pub fn run_kanban() -> Result<()> {
    intro("Athena Collaboration Kanban Board")?;

    let db = KanbanDB::new()?;
    db.init_default_board()?;

    let choice: usize = select("View, assign, and transition multi-agent tasks")
        .item(1, "View Kanban Board", "")
        .item(2, "Add new Task", "")
        .item(3, "Move Task status", "")
        .item(4, "Assign Task", "")
        .item(5, "Delete Task", "")
        .item(6, "Exit", "")
        .interact()?;

    match choice {
        1 => {
            let mut output = String::from("\n--- KANBAN BOARD ---\n");
            let cards = db.get_cards("default").unwrap_or_default();
            
            let columns = vec![
                ("col-todo", "TODO"),
                ("col-in-progress", "IN PROGRESS"),
                ("col-done", "DONE"),
            ];
            
            for (col_id, col_name) in &columns {
                output.push_str(&format!("\n  [{}]\n", col_name));
                let col_cards: Vec<_> = cards.iter().filter(|c| c.column_id == *col_id).collect();
                if col_cards.is_empty() {
                    output.push_str("    (No tasks)\n");
                } else {
                    for card in col_cards {
                        let assignee = card.assignee.as_deref().unwrap_or("unassigned");
                        output.push_str(&format!("    #{}: {} (Assigned to: {})\n", card.id, card.title, assignee));
                    }
                }
            }
            output.push_str("\n--------------------");
            outro(output)?;
        }
        2 => {
            let title: String = input("Enter task title").interact()?;
            let assignee_in: String = input("Enter assignee (optional)").interact()?;
            let assignee = if assignee_in.trim().is_empty() { None } else { Some(assignee_in.trim()) };
            
            let id = uuid::Uuid::new_v4().to_string();
            match db.create_card(&id, "default", "col-todo", &title, assignee) {
                Ok(_) => outro(format!("Successfully added task #{}", id))?,
                Err(e) => outro_cancel(e.to_string())?,
            }
        }
        3 => {
            let cards = db.get_cards("default").unwrap_or_default();
            if cards.is_empty() {
                outro_cancel("No tasks on the board.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select Task to move");
            for card in &cards {
                select_prompt = select_prompt.item(card.id.clone(), format!("#{} - {}", card.id, card.title), "");
            }
            let id: String = select_prompt.interact()?;
            
            let status: String = select("Select new status")
                .item("col-todo".to_string(), "Todo", "")
                .item("col-in-progress".to_string(), "In Progress", "")
                .item("col-done".to_string(), "Done", "")
                .interact()?;
            
            match db.move_card(&id, &status) {
                Ok(_) => outro(format!("Task #{} moved.", id))?,
                Err(e) => outro_cancel(e.to_string())?,
            }
        }
        4 => {
            let cards = db.get_cards("default").unwrap_or_default();
            if cards.is_empty() {
                outro_cancel("No tasks on the board.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select Task to assign");
            for card in &cards {
                select_prompt = select_prompt.item(card.id.clone(), format!("#{} - {}", card.id, card.title), "");
            }
            let id: String = select_prompt.interact()?;
            
            let assignee: String = input("Enter assignee").interact()?;
            match db.assign_card(&id, &assignee) {
                Ok(_) => outro(format!("Task #{} assigned to {}.", id, assignee))?,
                Err(e) => outro_cancel(e.to_string())?,
            }
        }
        5 => {
            let cards = db.get_cards("default").unwrap_or_default();
            if cards.is_empty() {
                outro_cancel("No tasks to delete.")?;
                return Ok(());
            }

            let mut select_prompt = select("Select Task to delete");
            for card in &cards {
                select_prompt = select_prompt.item(card.id.clone(), format!("#{} - {}", card.id, card.title), "");
            }
            let id: String = select_prompt.interact()?;

            match db.delete_card(&id) {
                Ok(_) => outro(format!("Task #{} deleted successfully.", id))?,
                Err(e) => outro_cancel(e.to_string())?,
            }
        }
        _ => { outro("Goodbye!")?; }
    }
    
    Ok(())
}

// Rust guideline compliant 2026-02-21
