#![feature(thread_id_value)]
#![feature(core_intrinsics)]
#![allow(warnings)] 
mod tcp_helper;



use csv::{Writer, WriterBuilder};
use dashmap::DashMap;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
/* enum command-line parsing stuff */
use ::trust::*;
use ::trust::green::*;
use structopt::StructOpt;
use std::collections::hash_map::DefaultHasher;
use std::fmt::format;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Write, Read, self};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::ptr::hash;
use std::{time::*, collections::HashMap};
//use std::time::{get_time, Timespec};
use std::cell::RefCell;
use std::rc::Rc;
use ::trust::thenjoin::*;
use rand::{distributions::{Distribution,Uniform}, seq::SliceRandom, RngCore};


#[derive(Debug, Serialize, Deserialize)]
pub struct HandShakeRequest {
    client_threads: usize,
    server_threads: usize,
    ops_per_req: usize,
    capacity: usize,
    key_type: KeyValueType,
    value_type: KeyValueType
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[derive(Eq, Hash, PartialEq)]
pub enum KeyValueType {
    Int(u64),
    // String(String),
    // Add more types as needed
}

fn receive_request_block<T: DeserializeOwned>(mut stream: &TcpStream) -> T {
    let mut buffer = [0; 1024 * 10];
    let bytes_read = stream.read(&mut buffer).unwrap();
    let request_json = String::from_utf8_lossy(&buffer[..bytes_read]).into_owned();
    // println!("request json: {}", request_json);
    let result = serde_json::from_str(&request_json).unwrap();
    // println!("done");
    return result;
}

fn receive_request_nonblock<T: DeserializeOwned>(mut stream: &TcpStream) -> Result<T, bool> {
    let mut buffer = [0; 1024 * 10];
    let mut read_result = stream.read(&mut buffer);
    let receive_result = match read_result {
        Ok(bytes) => {
            let request_json = String::from_utf8_lossy(&buffer[..bytes]).into_owned();
            // println!("The JSON is {}", request_json);
            let result = serde_json::from_str(&request_json).unwrap();
            Ok(result)
        },
        Err(_) => Err(false),
    };
    return receive_result;
}


fn main() {

    let num_cores_available = num_cpus::get();


    println!("The number of cores available is {}", num_cores_available);

    // Start server
    let address = "0.0.0.0:7879";
    let listener: TcpListener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(_) => panic!("Cannot bind"),
    };


    // Initializing parameters - Capacity of hashtable, no of socket fibers, no of operations per request, no of trustees (no of shards)
    let mut capacity = 0;
    let mut no_of_fibers = 0;
    let mut ops_per_req = 0;
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

        let handshake_request: HandShakeRequest = receive_request_block(&stream);
        println!("{}", handshake_request.capacity);
        capacity = handshake_request.capacity;
        no_of_fibers = handshake_request.client_threads;
        ops_per_req = handshake_request.ops_per_req;
        trustees_no = handshake_request.server_threads;
    }


    if (trustees_no >= num_cores_available) {
        println!("failed because trustees > num of cores");
        process::exit(1);
    }

    // Creating the thread pool - with the num cores available
    let num_cores_for_trustees = 1 + trustees_no;
    let num_cores_for_network = no_of_fibers;
    let num_cores_to_allocate = num_cores_available.min(num_cores_for_trustees + num_cores_for_network);
    let mut pool = ThreadPool::configure(num_cores_to_allocate, CpuAllocationStrategy::Compact);
    println!("Allocated pool with {} CPUs", num_cores_to_allocate);

    // No extra work on main thread of the program
    let main_thread_index = 0;
    // Entrusting the hashtables for server threads
    let mut entrusted_tables = vec![];
    for table_thread_index in 0..trustees_no {
        // let mut table:HashMap<u64, u64> = HashMap::with_capacity(1 << capacity);
        let mut table  = DashMap::with_capacity(capacity / no_of_fibers);
        println!("Trusting in thread {}", table_thread_index + 1);
        let entrusted_table = pool.workers[table_thread_index + 1].trustee().entrust(table);
        entrusted_tables.push(entrusted_table);
    }
    

    let server_thread_count = num_cores_available - trustees_no - 1;

    // Prefilling hashtable
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
        stream.set_nonblocking(true);
        let mut thread_index = trustees_no + 1 + server_thread_index;
        println!("Processing in thread {}", thread_index);
        let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables)| 
        {
            let fiber = grt().spawn(move || {
                process(stream, entrusted_tables.clone());
            });
            fiber
        }, (entrusted_tables.clone()));
        prefilling_fibers.push(fiber);
        server_thread_index = (server_thread_index + 1) % server_thread_count;
    }

    prefilling_fibers
        .into_iter()
        .map(|jh| jh.join())
        .collect::<Vec<_>>();
    


    // Doing work on hashtables
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
        stream.set_nonblocking(true);
        let mut thread_index = trustees_no + 1 + server_thread_index;
        println!("Processing in thread {}", thread_index);
        let fiber = pool.workers[thread_index].trustee_ref.as_ref().unwrap().apply_with(|trustee, (entrusted_tables)| 
        {
            let fiber = grt().spawn(move || {
                process(stream, entrusted_tables.clone())
            });
            fiber
        }, (entrusted_tables.clone()));
        work_fibers.push(fiber);
        server_thread_index = (server_thread_index + 1) % server_thread_count;
    }

    work_fibers
        .into_iter()
        .map(|jh| jh.join())
        .collect::<Vec<_>>();
    
    drop(entrusted_tables);
    drop(pool);
    println!("Experiment done");
}


#[derive(Debug, Serialize, Deserialize)]
enum Operation {
    Read { key: KeyValueType },
    Insert { key: KeyValueType, value: KeyValueType },
    Remove { key: KeyValueType },
    Increment { key: KeyValueType },
    Close
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    operations: Vec<Operation>,
}


#[derive(Debug, Serialize, Deserialize)]
struct OperationResults {
    results: Vec<OperationResult>,
}

#[derive(Debug, Serialize, Deserialize)]
enum OperationResult {
    Success(KeyValueType),
    Failure(String)
}

#[derive(Debug, Serialize, Deserialize)]
enum ResultData {
    String(String),
    Int(i32),
    // Add more types as needed
}

// Function to hash a string and return a number within the specified range
fn hash_str(value: &str, range: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let hash = hasher.finish();
    hash as usize % range
}


fn send_request<T: Serialize>(stream: &mut TcpStream, request: &T) {
    let request_json = serde_json::to_string(&request).expect("Failed to serialize request");
    stream.write_all(request_json.as_bytes()).expect("Failed to send request");
}

// Process function that gets operations in the TCP Stream and performs operations on the shards
fn process(mut stream: TcpStream, entrusted_tables: Vec<Trust<DashMap<KeyValueType, KeyValueType>>>,){
    loop {
        let request_result:Result<Request, bool> = receive_request_nonblock(&stream);
        if request_result.is_err() {
            grt().yield_now();
            continue;
        }
        let request = request_result.unwrap();
        let mut results = Vec::new();

        for operation in request.operations {
            let result = match operation {
                

                Operation::Read { key } => {
                    let tables_length = entrusted_tables.len();
                    let key_index = match key {
                        KeyValueType::Int(key_index) => key_index as usize,
                        // KeyValueType::String(ref string_value) => hash_str(&string_value, tables_length),
                    };
                    let shard_index = key_index as usize % tables_length;
                    let shard = &entrusted_tables[shard_index];
                    let result = shard.lazy_apply_with(move |hashmap, key| {
                        let result = hashmap.get(&key);
                        match result {
                            Some(value) => OperationResult::Success(value.clone()),
                            None => OperationResult::Failure("".to_string()),
                        }
                    }, key);
                    result
                }


                Operation::Insert { key, value } => {
                    let tables_length = entrusted_tables.len();
                    let key_index = match key {
                        KeyValueType::Int(key_index) => key_index as usize,
                        // KeyValueType::String(ref string_value) => hash_str(&string_value, tables_length),
                    };
                    let shard_index = key_index as usize % tables_length;
                    let shard = &entrusted_tables[shard_index];
                    let result = shard.lazy_apply_with(move |hashmap, (key, value)| {
                        let result = hashmap.insert(key, value.clone());
                        match result {
                            Some(old_value) => OperationResult::Success(old_value),
                            None => OperationResult::Success(value),
                        }
                    }, (key, value));
                    result
                }


                Operation::Remove { key } => {
                    let tables_length = entrusted_tables.len();
                    let key_index = match key {
                        KeyValueType::Int(key_index) => key_index as usize,
                        // KeyValueType::String(ref string_value) => hash_str(&string_value, tables_length),
                    };
                    let shard_index = key_index as usize % tables_length;
                    let shard = &entrusted_tables[shard_index];

                    let result = shard.lazy_apply_with(move |hashmap, key| {
                        let result = hashmap.remove(&key);
                        match result {
                            Some((key, value)) => OperationResult::Success(value.clone()),
                            None => OperationResult::Failure("".to_string()),
                        }
                    }, key);
                    result
                }

                Operation::Increment { key } => {
                    let tables_length = entrusted_tables.len();
                    let key_index = match key {
                        KeyValueType::Int(key_index) => key_index as usize,
                        // KeyValueType::String(ref string_value) => hash_str(&string_value, tables_length),
                    };
                    let shard_index = key_index as usize % tables_length;
                    let shard = &entrusted_tables[shard_index];

                    let result = shard.lazy_apply_with(move |hashmap, key| {
                        match hashmap.get_mut(&key) {
                            Some(entry) => {
                                match *entry {
                                    KeyValueType::Int(mut value) => {
                                        value = value + 1;
                                        OperationResult::Success(KeyValueType::Int(value + 1))
                                    },
                                    // KeyValueType::String(_) => OperationResult::IncrementFailure(String::from("Value is not an integer")),
                                }
                            }
                            None => OperationResult::Failure("".to_string()),
                        }
                    }, key);
                    result

                }
                Operation::Close => return,
            };
            
            results.push(result);
        }
        let final_results = results.iter().map(|result| {
            let value = result.join();
            value
        }).collect();

        let operation_results = OperationResults{
            results: final_results,
        };
        send_request(&mut stream, &operation_results);
    }
}
