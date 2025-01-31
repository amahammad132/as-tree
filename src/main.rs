extern crate ansi_term;
extern crate atty;
extern crate lscolors;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use lscolors::{LsColors, Style};

pub mod options;

#[derive(Debug, Default)]
pub struct PathTrie {
    // We rely on the sorted iteration order
    trie: BTreeMap<PathBuf, PathTrie>,
}

fn ansi_style_for_path(lscolors: &LsColors, path: &Path) -> ansi_term::Style {
    lscolors
        .style_for_path(&path)
        .map(Style::to_ansi_term_style)
        .unwrap_or_default()
}

impl PathTrie {
    fn contains_singleton_dir(&self) -> bool {
        if let Some(x) = self.trie.iter().next() {
            #[cfg(debug_assertions)] {
                eprintln!("\n\n");
                dbg!(&self.trie);
                dbg!(&self.trie.iter());
                dbg!(&x);
                eprintln!("<\x1b[93m{}\x1b[0m>", x.0.display());
            }
            
            if let Some(m) = x.1.trie.iter().next() {
                if m.1.trie.is_empty() {
                    return false;
                }
            }
        }
        
        // return true if the length is one and next node isn't empty
        self.trie.len() == 1 && !self.trie.iter().next().unwrap().1.trie.is_empty()
    }

    pub fn insert(&mut self, path: &Path) {
        let mut cur = self;
        for comp in path.iter() {
            cur = cur
                .trie
                .entry(PathBuf::from(comp))
                .or_insert_with(PathTrie::default);
        }
    }

    fn _print(
        &self,
        top: bool,
        prefix: &str,
        join_with_parent: bool,
        lscolors: &LsColors,
        parent_path: PathBuf,
        full_path: bool,
    ) {
        let normal_prefix = format!("{}│   ", prefix);
        let last_prefix = format!("{}    ", prefix);

        for (idx, (path, it)) in self.trie.iter().enumerate() {
            let current_path = parent_path.join(path);
            let style = ansi_style_for_path(&lscolors, &current_path);

            // let contains_singleton_dir = it.contains_singleton_dir();
            let contains_singleton_dir = top;

            let painted = match full_path {
                false => style.paint(path.to_string_lossy()),
                true => match contains_singleton_dir && !join_with_parent {
                    false => style.paint(current_path.to_string_lossy()),
                    true => style.paint(""),
                },
            };

            // If this folder only contains a single dir, we skip printing it because it will be
            // picked up and printed on the next iteration. If this is a full path (even if it
            // contains more than one directory), we also want to skip printing, because the full
            // path will be printed all at once (see painted above), not part by part.
            // If this is a full path however the prefix must be printed at the very beginning.
            let should_print = (contains_singleton_dir && !join_with_parent)
                || !contains_singleton_dir
                || !full_path;

            let newline = if contains_singleton_dir { "" } else { "\n" };
            let is_last = idx == self.trie.len() - 1;
            
            dbg!(&it, &current_path);
            dbg!(&contains_singleton_dir);
            dbg!(&full_path);
            dbg!(&join_with_parent);
            dbg!(&top);
            dbg!(&is_last);
            dbg!(&newline);
            dbg!(&parent_path.display());

            let next_prefix = if join_with_parent {
                let joiner = if full_path || top || parent_path == PathBuf::from("/") {
                    ""
                } else {
                    "/"
                };
                if should_print {
                    print!("{}{}{}", style.paint(joiner), painted, newline);
                }
                prefix
            } else if !is_last {
                if should_print {
                    print!("{}├── {}{}", prefix, painted, newline);
                }
                &normal_prefix
            } else {
                if should_print {
                    print!("{}└── {}{}", prefix, painted, newline);
                }
                &last_prefix
            };

            it._print(
                false,
                next_prefix,
                contains_singleton_dir,
                lscolors,
                current_path,
                full_path,
            )
        }
    }

    fn print(&self, lscolors: &LsColors, full_path: bool) {
        if self.trie.is_empty() {
            println!();
            return;
        }

        // This works because PathBuf::from(".").join(PathBuf::from("/")) == PathBuf::from("/")
        let current_path = PathBuf::from(".");
        let contains_singleton_dir = self.contains_singleton_dir();

        if !contains_singleton_dir {
            let style = ansi_style_for_path(&lscolors, &current_path);
            println!("{}", style.paint(current_path.to_string_lossy()));
        }

        self._print(
            true,
            "",
            contains_singleton_dir,
            &lscolors,
            current_path,
            full_path,
        )
    }
}

fn drain_input_to_path_trie<T: BufRead>(input: &mut T) -> PathTrie {
    let mut trie = PathTrie::default();

    for path_buf in input.lines().filter_map(Result::ok).map(PathBuf::from) {
        trie.insert(&path_buf)
    }

    trie
}

fn main() -> io::Result<()> {
    let options = options::parse_options_or_die();

    let trie = match &options.filename {
        None => {
            if atty::is(atty::Stream::Stdin) {
                eprintln!("Warning: reading from stdin, which is a tty.");
            }
            drain_input_to_path_trie(&mut io::stdin().lock())
        }
        Some(filename) => {
            let file = File::open(filename)?;
            let mut reader = BufReader::new(file);
            drain_input_to_path_trie(&mut reader)
        }
    };

    let lscolors = match &options.colorize {
        options::Colorize::Always => LsColors::from_env().unwrap_or_default(),
        options::Colorize::Auto => {
            if atty::is(atty::Stream::Stdout) {
                LsColors::from_env().unwrap_or_default()
            } else {
                LsColors::empty()
            }
        }
        options::Colorize::Never => LsColors::empty(),
    };

    trie.print(&lscolors, options.full_path);

    io::Result::Ok(())
}
