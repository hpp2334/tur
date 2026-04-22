mod raw;

fn main() {
    let tests: &[(&str, fn())] = &[("vello_counter_app", raw::vello_counter_app)];

    let mut passed = 0;
    let mut failed = 0;

    for (name, test_fn) in tests {
        print!("test {name} ... ");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test_fn())) {
            Ok(()) => {
                println!("ok");
                passed += 1;
            }
            Err(_) => {
                println!("FAILED");
                failed += 1;
            }
        }
    }

    println!("\n{} passed; {} failed;", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}
