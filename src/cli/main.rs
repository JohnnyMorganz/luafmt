use anyhow::{bail, format_err, Context, Result};
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use std::fs;
use std::io::{stdin, stdout, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;
use threadpool::ThreadPool;

use stylua_lib::{format_code, Config, Range};

mod config;
mod opt;
mod output_diff;

#[macro_export]
macro_rules! verbose_println {
    ($verbosity:expr, $str:expr) => {
        if $verbosity {
            println!($str);
        }
    };
    ($verbosity:expr, $str:expr, $($arg:tt)*) => {
        if $verbosity {
            println!($str, $($arg)*);
        }
    };
}

enum FormatResult {
    /// Operation was a success, the output was either written to a file or stdout. If diffing, there was no diff to create.
    Complete,
    /// There is a diff output. This stores the diff created
    Diff(Vec<u8>),
}

fn format_file(
    path: &Path,
    config: Config,
    range: Option<Range>,
    opt: &opt::Opt,
) -> Result<FormatResult> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    let before_formatting = Instant::now();
    let formatted_contents = format_code(&contents, config, range)
        .with_context(|| format!("Could not format file {}", path.display()))?;
    let after_formatting = Instant::now();

    verbose_println!(
        opt.verbose,
        "formatted {} in {:?}",
        path.display(),
        after_formatting.duration_since(before_formatting)
    );

    if opt.check {
        let diff = output_diff::output_diff(
            &contents,
            &formatted_contents,
            3,
            format!("Diff in {}:", path.display()),
            opt.color,
        )
        .context("Failed to create diff")?;

        match diff {
            Some(diff) => Ok(FormatResult::Diff(diff)),
            None => Ok(FormatResult::Complete),
        }
    } else {
        fs::write(path, formatted_contents)
            .with_context(|| format!("Could not write to {}", path.display()))?;
        Ok(FormatResult::Complete)
    }
}

/// Takes in a string and outputs the formatted version to stdout
/// Used when input has been provided to stdin
fn format_string(input: String, config: Config, range: Option<Range>) -> Result<FormatResult> {
    let out = &mut stdout();
    let formatted_contents =
        format_code(&input, config, range).context("Failed to format from stdin")?;
    out.write_all(&formatted_contents.into_bytes())
        .context("Could not output to stdout")?;
    Ok(FormatResult::Complete)
}

fn format(opt: opt::Opt) -> Result<i32> {
    if opt.files.is_empty() {
        bail!("error: no files provided");
    }

    // Load the configuration
    let config = config::load_config(&opt)?;

    // Create range if provided
    let range = if opt.range_start.is_some() || opt.range_end.is_some() {
        Some(Range::from_values(opt.range_start, opt.range_end))
    } else {
        None
    };

    let error_code = AtomicI32::new(0);

    let cwd = std::env::current_dir()?;

    // Build WalkBuilder with the files given, using any overrides set
    let mut walker_builder = WalkBuilder::new(&opt.files[0]);
    for file_path in &opt.files[1..] {
        walker_builder.add(file_path);
    }

    walker_builder
        .standard_filters(false)
        .hidden(true)
        .parents(true)
        .add_custom_ignore_filename(".styluaignore");

    let use_default_glob = match opt.glob {
        Some(ref globs) => {
            // Build overriders with any patterns given
            let mut overrides = OverrideBuilder::new(cwd);
            for pattern in globs {
                match overrides.add(pattern) {
                    Ok(_) => continue,
                    Err(err) => {
                        return Err(format_err!(
                            "error: cannot parse glob pattern {}: {}",
                            pattern,
                            err
                        ));
                    }
                }
            }
            let overrides = overrides.build()?;
            walker_builder.overrides(overrides);
            // We shouldn't use the default glob anymore
            false
        }
        None => true,
    };

    verbose_println!(
        opt.verbose,
        "creating a pool with {} threads",
        opt.num_threads
    );
    let pool = ThreadPool::new(opt.num_threads);
    let (tx, rx) = crossbeam_channel::unbounded();
    let opt = Arc::new(opt);
    let error_code = Arc::new(error_code);

    // Create a thread to handle the formatting output
    let read_error_code = error_code.clone();
    pool.execute(move || {
        for output in rx {
            match output {
                Ok(result) => match result {
                    FormatResult::Complete => (),
                    FormatResult::Diff(diff) => {
                        read_error_code.store(1, Ordering::SeqCst);

                        let stdout = stdout();
                        let mut handle = stdout.lock();
                        match handle.write_all(&diff) {
                            Ok(_) => (),
                            Err(err) => eprintln!("{:#}", err),
                        }
                    }
                },
                Err(err) => {
                    eprintln!("{:#}", err);
                    read_error_code.store(1, Ordering::SeqCst);
                }
            }
        }
    });

    let walker = walker_builder.build();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.is_stdin() {
                    let tx = tx.clone();
                    let opt = opt.clone();

                    pool.execute(move || {
                        if opt.check {
                            tx.send(Err(format_err!(
                                "warning: `--check` cannot be used whilst reading from stdin"
                            )))
                            .unwrap();
                        };

                        let mut buf = String::new();
                        match stdin().read_to_string(&mut buf) {
                            Ok(_) => tx.send(format_string(buf, config, range)),
                            Err(error) => {
                                tx.send(Err(error).context("Could not format from stdin"))
                            }
                        }
                        .unwrap();
                    });
                } else {
                    let path = entry.path().to_owned(); // TODO: stop to_owned?
                    let opt = opt.clone();
                    if path.is_file() {
                        // If the user didn't provide a glob pattern, we should match against our default one
                        // We should ignore the glob check if the path provided was explicitly given to the CLI
                        if use_default_glob && !opt.files.iter().any(|p| path == *p) {
                            lazy_static::lazy_static! {
                                static ref DEFAULT_GLOB: globset::GlobMatcher = globset::Glob::new("**/*.lua").expect("cannot create default glob").compile_matcher();
                            }
                            if !DEFAULT_GLOB.is_match(&path) {
                                continue;
                            }
                        }

                        let tx = tx.clone();
                        pool.execute(move || {
                            tx.send(format_file(&path, config, range, &opt)).unwrap()
                        });
                    }
                }
            }
            Err(error) => {
                eprintln!("{:#}", format_err!("error: could not walk: {}", error));
                error_code.store(1, Ordering::SeqCst);
            }
        }
    }

    drop(tx);
    pool.join();

    Ok(Arc::try_unwrap(error_code).unwrap().into_inner())
}

fn main() {
    let opt = opt::Opt::from_args();

    let exit_code = match format(opt) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{:#}", e);
            1
        }
    };

    std::process::exit(exit_code);
}
