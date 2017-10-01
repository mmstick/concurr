use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub fn file(inputs: &Arc<Mutex<VecDeque<(usize, String)>>>, path: &Path, ninputs: &mut usize) {
    match File::open(path) {
        Ok(file) => generic_read(file, inputs, ninputs),
        Err(why) => {
            eprintln!("concurr [CRITICAL]: unable to read inputs from '{:?}': {}", path, why);
        }
    }
}

pub fn stdin(inputs: &Arc<Mutex<VecDeque<(usize, String)>>>, ninputs: &mut usize) {
    let stdin = io::stdin();
    generic_read(stdin.lock(), inputs, ninputs);
}

fn generic_read<F: Read>(
    input: F,
    inputs: &Arc<Mutex<VecDeque<(usize, String)>>>,
    ninputs: &mut usize,
) {
    for line in BufReader::new(input).lines() {
        match line {
            Ok(input) => {
                let input = input.trim();
                if !input.is_empty() && !input.starts_with('#') {
                    let mut inputs = inputs.lock().unwrap();
                    inputs.push_back((*ninputs, input.to_owned()));
                    *ninputs += 1;
                }
            }
            Err(why) => {
                eprintln!("concurr [CRITICAL]: unable to read line from input: {}", why);
                break;
            }
        }
    }
}
