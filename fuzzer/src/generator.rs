// Nautilus
// Copyright (C) 2024  Daniel Teuchert, Cornelius Aschermann, Sergej Schumilo

mod python_grammar_loader;
use grammartec::context::Context;
use grammartec::tree::TreeLike;

use clap::Parser;
use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "generator",
    about = "Generate strings using a grammar. This can also be used to generate a corpus"
)]
struct Args {
    /// Path to grammar
    #[arg(short = 'g', value_name = "GRAMMAR")]
    grammar_path: String,
    /// Size of trees that are generated
    #[arg(short = 't', value_name = "DEPTH")]
    tree_depth: usize,
    /// Number of trees to generate
    #[arg(short = 'n', value_name = "NUMBER", default_value_t = 1)]
    number_of_trees: usize,
    /// Store output to files. This will create a folder called corpus containing one file for each generated tree.
    #[arg(short = 's')]
    store: bool,
    /// Be verbose
    #[arg(short = 'v')]
    verbose: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_arguments_and_defaults() {
        let args = Args::try_parse_from(["generator", "-g", "grammar.py", "-t", "100"]).unwrap();

        assert_eq!(args.grammar_path, "grammar.py");
        assert_eq!(args.tree_depth, 100);
        assert_eq!(args.number_of_trees, 1);
        assert!(!args.store);
        assert!(!args.verbose);
    }
}

fn main() {
    //Parse parameters
    let args = Args::parse();

    let grammar_path = args.grammar_path;
    let tree_depth = args.tree_depth;
    let number_of_trees = args.number_of_trees;
    let store = args.store;
    let verbose = args.verbose;

    let mut ctx = Context::new();
    //Create new Context and saved it
    if grammar_path.ends_with(".json") {
        let gf = File::open(grammar_path).expect("cannot read grammar file");
        let rules: Vec<Vec<String>> =
            serde_json::from_reader(&gf).expect("cannot parse grammar file");
        assert!(!rules.is_empty(), "rule file didn_t include any rules");
        let root = "{".to_string() + &rules[0][0] + "}";
        ctx.add_rule("START", root.as_bytes());
        for rule in rules {
            ctx.add_rule(&rule[0], rule[1].as_bytes());
        }
    } else if grammar_path.ends_with(".py") {
        ctx = python_grammar_loader::load_python_grammar(&grammar_path);
    } else {
        panic!("Unknown grammar type");
    }

    ctx.initialize(tree_depth);

    //Generate Tree
    if store {
        if Path::new("corpus").exists() {
        } else {
            fs::create_dir("corpus").expect("Could not create corpus directory");
        }
    }
    for i in 0..number_of_trees {
        let nonterm = ctx.nt_id("START");
        let len = ctx.get_random_len_for_nt(&nonterm);
        let generated_tree = ctx.generate_tree_from_nt(nonterm, len); //1 is the index of the "START" Node
        if verbose {
            println!("Generating tree {} from {}", i + 1, number_of_trees);
        }
        if store {
            let mut output =
                File::create(&format!("corpus/{}", i + 1)).expect("cannot create output file");
            generated_tree.unparse_to(&ctx, &mut output);
        } else {
            let stdout = io::stdout();
            let mut stdout_handle = stdout.lock();
            generated_tree.unparse_to(&ctx, &mut stdout_handle);
        }

        let mut of_tree = File::create("/tmp/test_tree.ron").expect("cannot create output file");
        of_tree
            .write_all(
                ron::ser::to_string(&generated_tree)
                    .expect("Serialization of Tree failed!")
                    .as_bytes(),
            )
            .expect("Writing to tree file failed");
    }
}
