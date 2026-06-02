#![no_main]
use libfuzzer_sys::fuzz_target;
use vte::{Params, Parser as VteParser, Perform as VtePerform};

/// Minimal performer that tracks dispatched events for comparison.
#[derive(Default, Clone)]
struct EventLog {
    events: Vec<EventKind>,
}

#[derive(Debug, Clone, PartialEq)]
enum EventKind {
    Print(char),
    Execute(u8),
    CsiDispatch { params_len: usize, final_byte: char },
    EscDispatch(u8),
    OscDispatch(usize),
    Hook(usize),
    Unhook,
    Put(u8),
}

impl VtePerform for EventLog {
    fn print(&mut self, c: char) {
        self.events.push(EventKind::Print(c));
    }
    fn execute(&mut self, byte: u8) {
        self.events.push(EventKind::Execute(byte));
    }
    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, c: char) {
        self.events.push(EventKind::CsiDispatch {
            params_len: params.len(),
            final_byte: c,
        });
    }
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        self.events.push(EventKind::EscDispatch(byte));
    }
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.events.push(EventKind::OscDispatch(params.len()));
    }
    fn hook(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, _c: char) {
        self.events.push(EventKind::Hook(params.len()));
    }
    fn unhook(&mut self) {
        self.events.push(EventKind::Unhook);
    }
    fn put(&mut self, byte: u8) {
        self.events.push(EventKind::Put(byte));
    }
}

fuzz_target!(|data: &[u8]| {
    // Run the VTE parser byte-by-byte (baseline)
    let mut baseline_parser = VteParser::new();
    let mut baseline_log = EventLog::default();
    for &byte in data {
        baseline_parser.advance(&mut baseline_log, byte);
    }

    // Run with ASCII pre-scan simulation:
    // Process ASCII runs (0x20..=0x7E) as direct prints when in ground state,
    // fall through to VTE for everything else.
    let mut opt_parser = VteParser::new();
    let mut opt_log = EventLog::default();
    let mut idx = 0;
    while idx < data.len() {
        if opt_parser.is_ground_state() {
            let remaining = &data[idx..];
            let ascii_len = remaining
                .iter()
                .position(|&b| b < 0x20 || b > 0x7E)
                .unwrap_or(remaining.len());
            if ascii_len > 0 {
                for &b in &data[idx..idx + ascii_len] {
                    opt_log.events.push(EventKind::Print(b as char));
                }
                idx += ascii_len;
                continue;
            }
        }
        opt_parser.advance(&mut opt_log, data[idx]);
        idx += 1;
    }

    // The two must produce identical event sequences
    assert_eq!(
        baseline_log.events.len(),
        opt_log.events.len(),
        "event count mismatch: baseline={}, optimized={}, input_len={}",
        baseline_log.events.len(),
        opt_log.events.len(),
        data.len()
    );
    assert_eq!(baseline_log.events, opt_log.events,
        "event sequence diverged on input of length {}", data.len());
});
