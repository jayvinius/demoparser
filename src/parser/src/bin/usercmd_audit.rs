//! Counts how user commands decode in a demo: full vs delta payloads, deltas that applied,
//! deltas with no baseline, failures by reason, and command-number gaps or regressions.
//!
//! Usage: usercmd_audit <demo.dem> [st|mt]

use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;
use parser::second_pass::usercmd_audit;
use std::env;
use std::fs::File;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("usage: usercmd_audit <demo.dem> [st|mt]");
    let mode = match args.get(2).map(String::as_str) {
        Some("st") => ParsingMode::ForceSingleThreaded,
        _ => ParsingMode::Normal,
    };
    let file = File::open(path).expect("open demo");
    let mmap = unsafe { MmapOptions::new().map(&file).expect("mmap") };
    let huf = create_huffman_lookup_table();
    let props = vec!["usercmd_input_history".to_string()];
    let inputs = ParserInputs {
        wanted_player_props: props.clone(),
        wanted_events: vec![],
        real_name_to_og_name: AHashMap::default(),
        wanted_other_props: vec![],
        parse_ents: true,
        wanted_players: vec![],
        wanted_ticks: vec![],
        parse_projectiles: false,
        parse_grenades: false,
        only_header: false,
        list_props: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        wanted_prop_states: AHashMap::default(),
        fallback_bytes: None,
    };
    usercmd_audit::reset();
    let t = Instant::now();
    let mut parser = Parser::new(inputs, mode);
    parser.parse_demo(&mmap).expect("parse");
    let secs = t.elapsed().as_secs_f64();

    let (counts, failures) = usercmd_audit::snapshot();
    println!("{path}  ({secs:.2}s)");
    let get = |c: usercmd_audit::Counter| counts.iter().find(|(k, _)| *k == c).map(|(_, v)| *v).unwrap_or(0);
    let delta = get(usercmd_audit::Counter::Delta).max(1) as f64;
    for (counter, value) in &counts {
        println!("  {:<18} {:>9}", format!("{counter:?}"), value);
    }
    println!(
        "  delta ok rate      {:>8.2}%",
        100.0 * get(usercmd_audit::Counter::DeltaOk) as f64 / delta
    );
    if !failures.is_empty() {
        println!("  failures by reason:");
        let mut sorted: Vec<_> = failures.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (reason, n) in sorted {
            println!("    {n:>8}  {reason}");
        }
    }
}
