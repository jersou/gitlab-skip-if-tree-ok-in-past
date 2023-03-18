// print msg only if SKIP_CI_VERBOSE env var == true
#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {{
        if std::env::var("SKIP_CI_VERBOSE")
            .map(|v| v == "true")
            .unwrap_or(false)
        {
            let out = format!($($arg)*);
            println!("SKIP-CI: {}", out.replace("\n", "\nSKIP-CI: "));
        }
    }};
}

pub fn red(msg: &str) {
    println!("\x1b[1;41;30mSKIP-CI:  {msg}  \x1b[0m");
}

pub fn green(msg: &str) {
    println!("\x1b[1;42;30mSKIP-CI:  {msg}  \x1b[0m");
}

pub fn yellow(msg: &str) {
    println!("\x1b[1;43;30mSKIP-CI:  {msg}  \x1b[0m");
}

// RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
#[cfg(test)]
mod tests {
    use crate::log::{green, red, yellow};
    use gag::BufferRedirect;
    use std::env;
    use std::io::Read;

    #[test]
    fn test_verbose_off() {
        temp_env::with_var("SKIP_CI_VERBOSE", None::<String>, || {
            // RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
            if env::var("RUST_TEST_NOCAPTURE")
                .map(|v| v == "1")
                .unwrap_or(false)
            {
                let mut std_output = String::new();
                {
                    let mut buf = BufferRedirect::stdout().unwrap();
                    verbose!("test");
                    buf.read_to_string(&mut std_output).unwrap();
                }
                assert_eq!(&std_output[..], "");
            }
        });
    }

    #[test]
    fn test_verbose_on() {
        temp_env::with_var("SKIP_CI_VERBOSE", Some("true"), || {
            // RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
            if env::var("RUST_TEST_NOCAPTURE")
                .map(|v| v == "1")
                .unwrap_or(false)
            {
                let mut std_output = String::new();
                {
                    let mut buf = BufferRedirect::stdout().unwrap();
                    verbose!("test");
                    buf.read_to_string(&mut std_output).unwrap();
                }
                assert_eq!(&std_output[..], "SKIP-CI: test\n");
            }
        });
    }

    #[test]
    fn test_red() {
        // RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
        if env::var("RUST_TEST_NOCAPTURE")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            let mut std_output = String::new();
            {
                let mut buf = BufferRedirect::stdout().unwrap();
                red("test");
                buf.read_to_string(&mut std_output).unwrap();
            }
            assert_eq!(
                &std_output[..],
                "\u{1b}[1;41;30mSKIP-CI:  test  \u{1b}[0m\n"
            );
        }
    }

    #[test]
    fn test_green() {
        // RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
        if env::var("RUST_TEST_NOCAPTURE")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            let mut std_output = String::new();
            {
                let mut buf = BufferRedirect::stdout().unwrap();
                green("test");
                buf.read_to_string(&mut std_output).unwrap();
            }
            assert_eq!(
                &std_output[..],
                "\u{1b}[1;42;30mSKIP-CI:  test  \u{1b}[0m\n"
            );
        }
    }

    #[test]
    fn test_yellow() {
        // RUST_TEST_NOCAPTURE=1 cargo test -- --test-threads=1
        if env::var("RUST_TEST_NOCAPTURE")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            let mut std_output = String::new();
            {
                let mut buf = BufferRedirect::stdout().unwrap();
                yellow("test");
                buf.read_to_string(&mut std_output).unwrap();
            }
            assert_eq!(
                &std_output[..],
                "\u{1b}[1;43;30mSKIP-CI:  test  \u{1b}[0m\n"
            );
        }
    }
}
