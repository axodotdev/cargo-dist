//! This is the same basic implementation found in miette, but we fork it here
//! so we can tune it to our own purposes and customize the panic hook itself.

use std::fmt::Write;

use backtrace::Backtrace;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("{0}{bt}", bt = Panic::backtrace())]
#[diagnostic(help("set the `RUST_BACKTRACE=1` environment variable to display a backtrace."))]
pub struct Panic(pub String);

impl Panic {
    fn backtrace() -> String {
        // FIXME: also handle `RUST_BACKTRACE=full`!
        if let Ok(var) = std::env::var("RUST_BACKTRACE") {
            if !var.is_empty() && var != "0" {
                // FIXME: I think this format is kinda ugly/verbose, we can maybe do better,
                // but I need to check what this looks like in shipping binaries (without local
                // debuginfo files helping the backtrace get symbolicated).
                const HEX_WIDTH: usize = std::mem::size_of::<usize>() + 2;
                // Padding for next lines after frame's address
                const NEXT_SYMBOL_PADDING: usize = HEX_WIDTH + 6;
                let mut backtrace = String::new();
                let trace = Backtrace::new();
                let frames = backtrace_ext::short_frames_strict(&trace).enumerate();
                for (idx, (frame, sub_frames)) in frames {
                    let ip = frame.ip();
                    let _ = write!(backtrace, "\n{:4}: {:2$?}", idx, ip, HEX_WIDTH);

                    let symbols = frame.symbols();
                    if symbols.is_empty() {
                        let _ = write!(backtrace, " - <unresolved>");
                        continue;
                    }

                    for (idx, symbol) in symbols[sub_frames].iter().enumerate() {
                        // Print symbols from this address,
                        // if there are several addresses
                        // we need to put it on next line
                        if idx != 0 {
                            let _ = write!(backtrace, "\n{:1$}", "", NEXT_SYMBOL_PADDING);
                        }

                        if let Some(name) = symbol.name() {
                            let _ = write!(backtrace, " - {}", name);
                        } else {
                            let _ = write!(backtrace, " - <unknown>");
                        }

                        // See if there is debug information with file name and line
                        if let (Some(file), Some(line)) = (symbol.filename(), symbol.lineno()) {
                            let _ = write!(
                                backtrace,
                                "\n{:3$}at {}:{}",
                                "",
                                file.display(),
                                line,
                                NEXT_SYMBOL_PADDING
                            );
                        }
                    }
                }
                return backtrace;
            }
        }
        "".into()
    }
}
