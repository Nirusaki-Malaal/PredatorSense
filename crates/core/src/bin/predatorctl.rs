use predator_core::{call, socket_path, Request};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request = match args.first().map(String::as_str) {
        None | Some("status") if args.len() <= 1 => Request::Status,
        Some("refresh") if args.len() == 1 => Request::Refresh,
        Some("cycle") if args.len() == 1 => Request::Cycle,
        Some("turbo") if args.len() == 1 => Request::Turbo,
        Some("restore") if args.len() == 1 => Request::Restore,
        Some("set") if args.len() == 3 => Request::Set {
            feature: args[1].clone(),
            value: args[2].clone(),
        },
        _ => {
            eprintln!("Usage: predatorctl [status|refresh|cycle|turbo|restore|set FEATURE VALUE]");
            std::process::exit(2);
        }
    };
    match call(&socket_path(), &request) {
        Ok(response) => {
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
            if !response.ok {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Daemon connection failed: {e}");
            std::process::exit(1);
        }
    }
}
