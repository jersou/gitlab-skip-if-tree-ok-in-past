mod artifact;
mod config;
mod find_last_job_ok;
mod git;
mod help;
mod jobs;
mod log;
mod process;
mod skip_ci_file;
mod trace;

#[cfg(not(tarpaulin_include))]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::args().len() <= 1 {
        verbose!("{}", help::get_version_msg());
        let config = config::config_from_env();
        let exit_code = process::process_with_exit_code(config).await;
        std::process::exit(exit_code);
    } else {
        help::print_help();
        std::process::exit(5);
    }
}
