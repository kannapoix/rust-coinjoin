use std::env;
use std::fs;
use std::path::Path;
use std::io;
use std::io::Write;
use core::str::FromStr;
use std::fs::File;

use serde::{Serialize, Deserialize};
use serde_json;
use serde_json::Number;
use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use rand::Rng;

use bdk::wallet::{Wallet, AddressIndex};
use bdk::database::{MemoryDatabase};
use bdk::blockchain::{noop_progress, ElectrumBlockchain};
use bdk::electrum_client::Client;
use bdk::Error;
use bdk::bitcoin;
use bdk::bitcoin::hashes::hex::FromHex;
use bdk::bitcoin::util::{bip32, psbt::Input};
use bdk::bitcoin::Network;
use bdk::bitcoin::OutPoint;
use bdk::bitcoin::consensus::encode::{serialize, serialize_hex, deserialize};
use bdk::bitcoin::secp256k1::{Secp256k1};
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};

type InnerWallet = Wallet<ElectrumBlockchain, MemoryDatabase>;
type PSBT = bdk::bitcoin::util::psbt::PartiallySignedTransaction;

const ENV_HOST: &str = "HOST";
const ENV_NETWORK: &str = "NETWORK";

const MNEMONIC_DIR: &str = "./data/client/mnemonic";
const PSBT_INPUT_DIR: &str = "./data/client/psbt_inputs";
const UTXO_DIR: &str = "./data/client/utxos";
const MIXER_MNEMONIC_PATH: &str = "./data/mixer/mnemonic/alice.mnemonic";
const PSBT_PATH: &str = "./data/psbt.txt";


// UTXO
#[derive(Debug, Deserialize)]
struct OutputSet {
    outpoint: OutPoint,
    input: bitcoin::util::psbt::Input,
}
#[derive(Debug, Deserialize)]
struct Utxo {
    outpoint: String,
    txout: Txout,
    keychain: String
}

#[derive(Debug, Deserialize)]
struct Txout {
    value: Number,
    script_pubkey: String
}

fn setup_client_wallets() -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
    let mut mnemonics:Vec<Mnemonic> = Vec::new();
    for file_name in Path::new(MNEMONIC_DIR).read_dir().expect("read_dir call failed") {
        if let Ok(file_name) = file_name {
            let file_path = file_name.path();
            match fs::read_to_string(&file_path) {
                Ok(string) => {
                    println!("Read from {:?}", file_path);
                    let mnemonic = Mnemonic::parse(&string).unwrap();
                    mnemonics.push(mnemonic)
                },
                Err(e) => {
                    eprintln!("Faild to read file {}: {}", &file_path.to_str().unwrap_or("unknown file"), e);
                    std::process::exit(1);
                }
            }
        }
    }

    let mut clients:Vec<String> = Vec::new();
    for mnemonic in mnemonics.iter() {
        let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
        let xprv = xkey.into_xprv(Network::Regtest).unwrap();
        clients.push(xprv.to_string());
    }
    init_client_wallet(Network::Regtest, "127.0.0.1:50001", &clients)
}

// dump_utxos dumps utxo data of each client wallet into local file. JSON schema is following.
// e.g. {"outpoint":"b78fb014ff8d7bbee82a393a371f852380e6007e838b1c62dc5d9c12491d08a4:1","txout":{"value":2000000000,"script_pubkey":"00143c45afd830fe843a91136a9f7df3064c2e0778b9"},"keychain":"External"}
fn dump_utxos() {
    let wallets = setup_client_wallets();

    for (i, wallet) in wallets.iter().enumerate() {
        wallet.sync(noop_progress(), None).unwrap();
        println!("wallet {:?} has {:?}", wallet.get_address(AddressIndex::Peek(0)).unwrap(), wallet.get_balance().unwrap());
        // TODO: select utxo to be used as Input
        let local_utxo = &wallet.list_unspent().unwrap()[0];
        let json = serde_json::to_vec(&local_utxo).unwrap();

        fs::create_dir_all(UTXO_DIR).unwrap();
        let mut file = File::create(format!("{}/{}.json", UTXO_DIR, i)).unwrap();
        file.write_all(&json).unwrap();
    }
}

fn dump_psbt_input() {
    const UTXO_DIR: &str = "./data/client/utxos";
    let mut utxos: Vec<bdk::LocalUtxo> = Vec::new();
    for file_name in Path::new(UTXO_DIR).read_dir().expect("read_dir call failed") {
        if let Ok(file_name) = file_name {
            let file_path = file_name.path();
            match fs::read_to_string(&file_path) {
                Ok(string) => {
                    println!("Read from {:?}", file_path);
                    let utxo: bdk::LocalUtxo = serde_json::from_str(&string).unwrap();
                    utxos.push(utxo)
                },
                Err(e) => {
                    eprintln!("Faild to read file {}: {}", &file_path.to_str().unwrap_or("unknown file"), e);
                    std::process::exit(1);
                }
            }
        }
    }

    let pubkey_wallets = setup_client_wallets();
    for wallet in pubkey_wallets.iter() {
        wallet.sync(noop_progress(), None).unwrap();
        for i in 0..5 {
            match wallet.get_psbt_input(utxos[i].clone(), None, false) {
                Ok(input) => {
                    println!("UTXO found: {:?}", &input);
                    let psbt = serialize_hex(&input);
                    fs::create_dir_all(PSBT_INPUT_DIR).unwrap();
                    let mut file = File::create(format!("{}/{}.txt", PSBT_INPUT_DIR, i)).unwrap();
                    file.write_all(psbt.as_bytes()).unwrap();
                },
                Err(err) => {
                    println!("Error: {:?}", err)
                },
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CoinJoinInput {
    outpoint: String,
    psbt_input: String
}

#[post("/input")]
async fn record_input(input: web::Json<CoinJoinInput>) -> actix_web::Result<HttpResponse> {
    let temp_name: i16 = rand::thread_rng().gen_range(1000..10000);

    fs::create_dir_all(UTXO_DIR).unwrap();
    let mut file = File::create(format!("{}/{}.json", UTXO_DIR, temp_name)).unwrap();
    file.write_all(serde_json::to_string(&input).unwrap().as_bytes()).unwrap();
    Ok(HttpResponse::Ok().finish())
}

#[get("/utxo")]
async fn record_utxo() -> actix_web::Result<HttpResponse> {
    dump_utxos();
    Ok(HttpResponse::Ok().finish())
}

#[get("/psbt-input")]
async fn record_psbt_input() -> actix_web::Result<HttpResponse> {
    dump_psbt_input();
    Ok(HttpResponse::Ok().finish())
}

#[get("/psbt")]
async fn generate_psbt() -> actix_web::Result<HttpResponse> {
    let env_network = env::var(ENV_NETWORK).unwrap();
    let network = match env_network.as_str() {
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        _ => panic!("Given network is {:?}", env_network),
    };
    let host = env::var(ENV_HOST).unwrap();

    // Initialize mixer wallet
    let mixer_vec  = match fs::read_to_string(Path::new(MIXER_MNEMONIC_PATH))  {
        Ok(string) => {
            let mnemonic = Mnemonic::parse(&string).unwrap();
            let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
            let xprv = xkey.into_xprv(network).unwrap();
            init_client_wallet(network, &host, &vec![xprv.to_string()])
        },
        Err(e) => {
            eprintln!("Faild to read file {}: {}", MIXER_MNEMONIC_PATH, e);
            std::process::exit(1);
        }
    };
    let mixer = &mixer_vec[0];
    mixer.sync(noop_progress(), None).unwrap();
    println!("Mixer {:?} has {:?}", mixer.get_address(AddressIndex::Peek(0)).unwrap(), mixer.get_balance().unwrap());

    // TODO: Finally get utxo from client
    const JSON_DIR: &str = "./data/client/utxos";
    let mut utxos: Vec<bdk::LocalUtxo> = Vec::new();
    for file_name in Path::new(JSON_DIR).read_dir().expect("read_dir call failed") {
        if let Ok(file_name) = file_name {
            let file_path = file_name.path();
            match fs::read_to_string(&file_path) {
                Ok(string) => {
                    println!("Read from {:?}", file_path);
                    let utxo: bdk::LocalUtxo = serde_json::from_str(&string).unwrap();
                    utxos.push(utxo)
                },
                Err(e) => {
                    eprintln!("Faild to read file {}: {}", &file_path.to_str().unwrap_or("unknown file"), e);
                    std::process::exit(1);
                }
            }
        }
    }

    let psbt_inputs = Path::new(PSBT_INPUT_DIR)
        .read_dir()
        .map(|res| res.map(|e| {
            e.and_then(|e| {
                match fs::read_to_string(&e.path()) {
                    Ok(data) => {
                        let psbt_input_string: Vec<u8> = FromHex::from_hex(&data).unwrap();
                        Ok(deserialize::<Input>(&psbt_input_string).unwrap())
                    },
                    Err(e) => {
                        eprintln!("Faild to read file: {}",  e);
                        std::process::exit(1);
                    }
                }
            })
        }))
        .map(|resp| resp.collect::<std::result::Result<Vec<Input>, io::Error>>())
        .unwrap_or_else(|err| {
            eprintln!("Faild to read directory: {}",  err);
            std::process::exit(1);
        })
        .unwrap_or_else(|err| {
            eprintln!("Faild to read file: {}",  err);
            std::process::exit(1);
        });

    let output_sets: Vec<OutputSet> = utxos
        .iter()
        .map(|utxo| {
            let input = psbt_inputs
                .iter()
                .find(|input| input.non_witness_utxo.as_ref().unwrap().txid() == utxo.outpoint.txid ).unwrap();

            OutputSet{ outpoint: utxo.outpoint, input: input.clone()}
        })
        .collect::<Vec<OutputSet>>();

    let (psbt, _) = {
        let mut builder = mixer.build_tx();
        builder
            .fee_rate(bdk::FeeRate::from_sat_per_vb(10.0))
            .do_not_spend_change();

        for _ in 0..5 {
            builder.add_recipient(mixer.get_address(AddressIndex::New).unwrap().script_pubkey(), 5_000);
        }

        for output_set in &output_sets {
            // Register outpoint and psbt input
            // outpoint is pointing utxo which used in coinjoin tx
            // psbt_input is input of coinjoin tx which we are going to create
            // builder.add_foreign_utxo(into_rust_bitcoin_output(&psbt_input.outpoint), psbt_input.input.clone(), 32).unwrap();// check about weight
            builder.add_foreign_utxo(into_rust_bitcoin_output(&output_set.outpoint), output_set.input.clone(), 32).unwrap();// check about weight
        }
        builder.finish().unwrap()
    };

    let hex_psbt = serialize_hex(&psbt);
    println!("{:?}", hex_psbt);
    let mut file = std::fs::File::create(PSBT_PATH).unwrap();
    file.write_all(hex_psbt.as_bytes()).unwrap();
 
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(
            web::scope("/api")
                .service(
                    web::scope("/v1")
                    .service(record_input)
                    .service(record_utxo)
                    .service(record_psbt_input)
                    .service(generate_psbt)
                )
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

fn merge_psbts(mut psbts: Vec<PSBT>) -> Vec<PSBT> {
    return if psbts.len() == 1 {
        psbts
    } else {
        let mut merged_psbt = psbts.pop().unwrap();
        merged_psbt.merge(psbts.pop().unwrap()).unwrap();
        psbts.push(merged_psbt);
        merge_psbts(psbts)
    };
}

fn list_signed_txs(mut psbt: PSBT, wallets: &Vec<InnerWallet>) -> Vec<PSBT> {
    let mut psbts = Vec::new();
    for wallet in wallets.iter() {
        wallet.sync(noop_progress(), None);
        let mut psbt_for_each_wallet = psbt.clone();
        match wallet.sign(&mut psbt_for_each_wallet, bdk::SignOptions::default()) {
            Ok(result) => {
                println!("Sign status is {}", result);
            },
            Err(error) => {
                println!("My Error {:?}", error)
            }
        }
        psbts.push(psbt_for_each_wallet.clone())
    };
    psbts
}

fn init_client_wallet(network: bitcoin::Network, electrum_endpoint: &str, clients: &Vec<String>) -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
    clients.iter().map( |client| {
        let descriptors = prepare_descriptor(client);
        return generate_wallet(&descriptors[0], &descriptors[1], network, electrum_endpoint).unwrap();
    }).collect()
}

fn init_client_pubkey_wallet(network: bitcoin::Network, electrum_endpoint: &str, clients: &Vec<String>) -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
    clients.iter().map( |client| {
        let descriptors = prepare_public_descriptor(client);
        return generate_wallet(&descriptors[0], &descriptors[1], network, electrum_endpoint).unwrap();
    }).collect()
}

fn prepare_descriptor(base: &str) -> [String;2] {
    let descriptor = format!("wpkh({}/84'/1'/0'/0/*)", base);
    let change_descriptor = format!("wpkh({}/84'/1'/0'/1/*)", base);
    return [descriptor, change_descriptor];
}

fn prepare_public_descriptor(base: &str) -> [String;2] {
    let descriptor = format!("wpkh({})", base);
    let change_descriptor = format!("wpkh({})", base);
    return [descriptor, change_descriptor];
}

fn into_rust_bitcoin_output(out_point: &OutPoint) -> bitcoin::blockdata::transaction::OutPoint {
    bitcoin::blockdata::transaction::OutPoint{ txid: out_point.txid, vout: out_point.vout }
}

fn generate_wallet(descriptor: &str, change_descriptor: &str, network: bitcoin::Network, electrum_endpoint: &str) -> std::result::Result<InnerWallet, Error> {
    let client = Client::new(electrum_endpoint).unwrap();
    Wallet::new(
        descriptor,
        Some(change_descriptor),
        network,
        MemoryDatabase::default(),
        ElectrumBlockchain::from(client)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_psbt() {
        let wallets = setup_client_wallets();

        let hex_psbt = fs::read_to_string(PSBT_PATH).unwrap();
        let psbt = bdk::bitcoin::consensus::deserialize::<bdk::bitcoin::util::psbt::PartiallySignedTransaction>(&<Vec<u8> as bdk::bitcoin::hashes::hex::FromHex>::from_hex(&hex_psbt).unwrap()).unwrap();

        let psbts = list_signed_txs(psbt, &wallets);
        match merge_psbts(psbts).pop() {
            Some(psbt) => {
                println!("Finalized PSBT {:?}", &serialize_hex(&psbt));
                println!("Finalized tx extracted from PSBT {:?}", &serialize_hex(&psbt.extract_tx()));
            },
            None => println!("Can not get first item.")
        };
    }
}
