mod raw;
mod vello_app;

fn main() {
    let tests: &[(&str, fn())] = &[
        ("vello_counter_app", raw::vello_counter_app),
        ("vello_dpr_1_renders_colors", raw::vello_dpr_1_renders_colors),
        ("vello_dpr_1_5_renders_colors", raw::vello_dpr_1_5_renders_colors),
        ("vello_dpr_2_renders_colors", raw::vello_dpr_2_renders_colors),
        ("vello_dpr_3_renders_colors", raw::vello_dpr_3_renders_colors),
        ("vello_image_renders", raw::vello_image_renders),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (name, test_fn) in tests {
        print!("test {name} ... ");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(test_fn)) {
            Ok(()) => {
                println!("ok");
                passed += 1;
            }
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    format!("{e:?}")
                };
                println!("FAILED\n  {msg}");
                failed += 1;
            }
        }
    }

    println!("\n{} passed; {} failed;", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}
