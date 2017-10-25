use super::Inputs;
use concurr::InsertJob;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Arc;

pub fn file(inputs: &Arc<Inputs>, path: &Path, ninputs: &mut usize) {
    match File::open(path) {
        Ok(file) => generic_read(file, inputs, ninputs),
        Err(why) => {
            eprintln!("concurr [CRITICAL]: unable to read inputs from '{:?}': {}", path, why);
        }
    }
}

pub fn stdin(inputs: &Arc<Inputs>, ninputs: &mut usize) {
    let stdin = io::stdin();
    generic_read(stdin.lock(), inputs, ninputs);
}

fn generic_read<F: Read>(input: F, inputs: &Arc<Inputs>, ninputs: &mut usize) {
    for line in BufReader::new(input).lines() {
        match line {
            Ok(input) => {
                let input = input.trim();
                if !input.is_empty() && !input.starts_with('#') {
                    inputs.insert_job(*ninputs, input.to_owned());
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
