use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
};

pub fn convert_string_to_int(string: String) -> usize {
    let string = string.trim();
    let parsed_number = match string.parse::<usize>() {
        Ok(no) => no,
        Err(_) => 0,
    };
    return parsed_number;
}

pub fn read_setup(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) -> String {
    let mut input = String::new();
    match reader.read_line(&mut input) {
        Ok(_) => 0,
        Err(_) => 0,
    };
    let input: String = input.trim().to_owned();
    return input;
}

pub fn read_command(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>, ops_st: usize) -> Vec<u8> {
    let mut input = vec![0u8; 9 * ops_st];
    reader.read_exact(&mut input);
    return input;
}

pub fn write_string(stream: &mut TcpStream, output: String) {
    match stream.write(output.as_bytes()) {
        Ok(_) => 0,
        Err(_) => 0,
    };
}
