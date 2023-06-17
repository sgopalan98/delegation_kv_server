#![feature(thread_id_value)]
#![feature(core_intrinsics)]
#![allow(warnings)] 
mod tcp_helper;



use csv::{Writer, WriterBuilder};
use serde::{Serialize, Deserialize};
/* enum command-line parsing stuff */
use ::trust::*;
use ::trust::green::*;
use structopt::StructOpt;
use std::fmt::format;
use std::fs::OpenOptions;
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::ptr::hash;
use std::{time::*, collections::HashMap};
//use std::time::{get_time, Timespec};
use std::cell::RefCell;
use std::rc::Rc;
use ::trust::thenjoin::*;
use rand::{distributions::{Distribution,Uniform}, seq::SliceRandom, RngCore};



fn main() {


    let num_cores_available = num_cpus::get();


    println!("The number of cores available is {}", num_cores_available);
    // Creating the thread pool - with the num cores available
    let mut pool = ThreadPool::configure(num_cores_available, CpuAllocationStrategy::Compact);
    println!("Created pool");

    // Start server
    let address = "0.0.0.0:7879";
    let listener: TcpListener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(_) => panic!("Cannot bind"),
    };


    // Experiment start
    let mut capacity = 0;
    let mut no_of_fibers = 0;
    let mut ops_st = 0;
    let mut trustees_no = 0;

    for stream in listener.incoming().take(1) {
        let mut stream = match stream {
            Ok(tcp_stream) => tcp_stream,
            Err(_) => panic!("NO STREAM"),
        };
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(stream) => stream,
            Err(_) => panic!("Cannot clone stream"),
        });
        let command = tcp_helper::read_setup(&mut stream, &mut reader);
        let command_units = command.split_whitespace().collect::<Vec<_>>();
        let trustees_no_command = command_units[0].to_owned();
        let capacity_command = command_units[1].to_owned();
        let no_of_fibers_command = command_units[2].to_owned();
        let ops_st_command = command_units[3].to_owned();
        println!("{} {} {} {}\n", trustees_no_command, capacity_command, no_of_fibers_command, ops_st_command);
        trustees_no = tcp_helper::convert_string_to_int(trustees_no_command);
        capacity = tcp_helper::convert_string_to_int(capacity_command);
        no_of_fibers = tcp_helper::convert_string_to_int(no_of_fibers_command);
        ops_st = tcp_helper::convert_string_to_int(ops_st_command);
    }


    if (trustees_no >= num_cores_available) {
        println!("failed because trustees > num of cores");
        process::exit(1);
    }

    let server_thread_count = num_cores_available - trustees_no;

    // Entrusting the hashtables for server threads
    let mut entrusted_tables = vec![];
    for table_thread_index in 0..trustees_no {
        let mut table:HashMap<u64, u64> = HashMap::with_capacity(capacity);
        let entrusted_table = pool.workers[table_thread_index].trustee().entrust(table);
        entrusted_tables.push(entrusted_table);
    }


    let mut prefiller_streams = vec![];
    for stream in listener.incoming().take(no_of_fibers) {
        let stream = match stream {
            Ok(stream) => stream,
            Err(_) => panic!("Cannot obtain stream"),
        };
        prefiller_streams.push(stream);
    }

    let mut prefilling_fibers = vec![];
    let mut server_thread_index = 0;

    for stream in prefiller_streams {
        let mut thread_index = trustees_no + server_thread_index;
        
        let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables, ops_st)| 
        {
            let fiber = grt().spawn(move || {
                process(stream, entrusted_tables.clone(), ops_st);
            });
            fiber
        }, (entrusted_tables.clone(), ops_st));
        prefilling_fibers.push(fiber);
        server_thread_index = (server_thread_index + 1) % server_thread_count;
    }

    prefilling_fibers
        .into_iter()
        .map(|jh| jh.join())
        .collect::<Vec<_>>();
    

    // let mut prefilling_fibers = vec![];
    // let mut server_thread_index = 0;
    // for stream in listener.incoming().take(no_of_fibers) {
    //     let stream = match stream {
    //         Ok(stream) => stream,
    //         Err(_) => panic!("Cannot obtain stream"),
    //     };
    //     let mut thread_index = trustees_no + server_thread_index;
        
    //     let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables, ops_st)| 
    //     {
    //         let fiber = grt().spawn(move || {
    //             process(stream, entrusted_tables.clone(), ops_st);
    //         });
    //         fiber
    //     }, (entrusted_tables.clone(), ops_st));
    //     prefilling_fibers.push(fiber);
    //     server_thread_index = (server_thread_index + 1) % server_thread_count;
    // }


    let mut work_streams = vec![];
    for stream in listener.incoming().take(no_of_fibers) {
        let stream = match stream {
            Ok(stream) => stream,
            Err(_) => panic!("Cannot obtain stream"),
        };
        work_streams.push(stream);
    }

    let mut work_fibers = vec![];
    let mut server_thread_index = 0;

    for stream in work_streams {
        let mut thread_index = trustees_no + server_thread_index;
        
        let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables, ops_st)| 
        {
            let fiber = grt().spawn(move || {
                process(stream, entrusted_tables.clone(), ops_st);
            });
            fiber
        }, (entrusted_tables.clone(), ops_st));
        work_fibers.push(fiber);
        server_thread_index = (server_thread_index + 1) % server_thread_count;
    }

    work_fibers
        .into_iter()
        .map(|jh| jh.join())
        .collect::<Vec<_>>();
    



    // let mut work_fibers = vec![];
    // let mut server_thread_index = 0;
    // for stream in listener.incoming().take(no_of_fibers) {
    //     println!("getting stream");
    //     let stream = match stream {
    //         Ok(stream) => stream,
    //         Err(_) => panic!("Cannot obtain stream"),
    //     };

    //     let mut thread_index = trustees_no + server_thread_index;
    //     let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables, ops_st)| 
    //     {
    //         let fiber = grt().spawn(move || {
    //             process(stream, entrusted_tables.clone(), ops_st);
    //         });
    //         fiber
    //     }, (entrusted_tables.clone(), ops_st));

    //     work_fibers.push(fiber);
    //     server_thread_index = (server_thread_index + 1) % server_thread_count;
    // }

    
    // work_fibers
    //     .into_iter()
    //     .map(|jh| jh.join())
    //     .collect::<Vec<_>>();
    drop(entrusted_tables);
    drop(pool);
    println!("Experiment done");
}

fn process(mut stream: TcpStream, entrusted_tables: Vec<Trust<HashMap<u64, u64>>>, ops_st: usize) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => panic!("Cannot clone stream"),
    });
    loop {
        let command_u8s = tcp_helper::read_command(&mut stream, &mut reader, ops_st);

        if command_u8s.len() == 0 {
            println!("Not receiving anything");
            continue;
        }
        let mut error_codes = vec![0u8; ops_st];
        let mut done = 0;
        for index in 0..ops_st {
            let start_index = 9 * index;
            let end_index = 9 * index + 9;
            let operation = command_u8s[start_index];
            let key_u8s = &command_u8s[(start_index + 1)..end_index];
            let key = u64::from_be_bytes(
                match key_u8s.try_into() {
                    Ok(key) => key,
                    Err(_) => panic!("Cannot convert slice to array"),
                },
            );
            let tables_length = entrusted_tables.len();
            let shard_index = key as usize % tables_length;
            let shard = &entrusted_tables[shard_index];

            let error_code: u8;
            // CLOSE
            if operation == 0 {
                done = 1;
                break;
            }
            // GET
            else if operation == 1 {
                let error_code = shard.apply(move |hashmap| {
                    let result = hashmap.get(&key);
                    result.is_some()
                });
                let error_code_byte = match error_code {
                    true => 0,
                    false => 1
                };
                error_codes[index] = error_code_byte;
            }
            // INSERT
            else if operation == 2 {
                let error_code = shard.apply(move |hashmap| {
                    let result = hashmap.insert(key, 0);
                    result.is_none()
                });
                let error_code_byte = match error_code {
                    true => 0,
                    false => 1
                };
                error_codes[index] = error_code_byte;
            }
            // REMOVE
            else if operation == 3 {
                let error_code = shard.apply(move |hashmap| {
                    let result = hashmap.remove(&key);
                    result.is_some()
                });
                let error_code_byte = match error_code {
                    true => 0,
                    false => 1
                };
                error_codes[index] = error_code_byte;
            }
            // UPDATE
            else if operation == 4 {
                let error_code = shard.apply(move |hashmap| {
                    hashmap.get_mut(&key).map(|mut v| *v += 1).is_some()
                });
                let error_code_byte = match error_code {
                    true => 0,
                    false => 1
                };
                error_codes[index] = error_code_byte;
            } 
            else {
            
            }
        }
        stream.write(&error_codes);
        if done == 1 {
            return;
        }
    }
}
