use crate::config;
use crate::db::Database;
use crate::ui::agents::print_agents_table;

pub fn run() -> Result<(), String> {
    let db = Database::open(&config::db_path())?;
    let agents = db.list_agents()?;
    print_agents_table(&agents);
    Ok(())
}
