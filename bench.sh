set -x

BIN=./target/release/bustle_trust
OUT=./results

mkdir -p "$OUT"
cargo build --release --bin bustle_trust

function bench {
    date

    file="$OUT/$1.csv"
    rm $file
    header="Capacity, Server threads, Client threads, Time taken in seconds,Throughput in MOPs"
    echo $header > $file

    for (( server_threads=1; server_threads<=$3; server_threads++ ));
    do
        for (( client_threads=1; client_threads<=$4; client_threads++ ));
        do
            $BIN -t $1 -i $2 -s $server_threads -c $client_threads
        done
    done

    echo '$1 done' 
}

capacity=$1
server_threads=$2
client_threads=$3


bench ReadHeavy $capacity $server_threads $client_threads
bench Exchange $capacity $server_threads $client_threads
bench RapidGrow $capacity $server_threads $client_threads
