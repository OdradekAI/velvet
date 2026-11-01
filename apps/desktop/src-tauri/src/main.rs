fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--velvet-recovery-window")
    {
        let root = arguments
            .get(index + 1)
            .filter(|argument| *argument == "--velvet-recovery-root")
            .and_then(|_| arguments.get(index + 2))
            .map(std::path::PathBuf::from);
        if let Some(root) = root {
            velvet_desktop::run_native_recovery_window(root);
            return;
        }
        eprintln!("Velvet recovery window arguments are incomplete");
        std::process::exit(2);
    }
    velvet_desktop::run();
}
