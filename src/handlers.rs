use actix_web::{get, post, web, HttpResponse};
use core::str::FromStr;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

use bdk::bitcoin;
use bdk::bitcoin::consensus::encode::{deserialize, serialize_hex};
use bdk::bitcoin::hashes::hex::FromHex;
use bdk::bitcoin::util::{address::Address, psbt::Input};
use bdk::bitcoin::Network;
use bdk::bitcoin::OutPoint;
use bdk::blockchain::noop_progress;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};
use bdk::wallet::AddressIndex;

use crate::utils;

const ENV_HOST: &str = "HOST";
const ENV_NETWORK: &str = "NETWORK";

const INPUT_DIR: &str = "./data/client/inputs";
const MIXER_MNEMONIC_PATH: &str = "./data/mixer/mnemonic/alice.mnemonic";
const SERVER_INPUT_DIR: &str = "./data/client/server_inputs";
const OUTPUT_DIR: &str = "./data/client/outputs";
const PSBT_PATH: &str = "./data/psbt.txt";

#[derive(Serialize, Deserialize, Debug)]
struct CoinJoinInput {
    outpoint: String,
    psbt_input: String,
}

#[post("/input")]
async fn record_input(input: web::Json<CoinJoinInput>) -> actix_web::Result<HttpResponse> {
    // NOTE: Maybe there are better numbering method
    let temp_name: i16 = rand::thread_rng().gen_range(1000..10000);
    fs::create_dir_all(INPUT_DIR).unwrap();
    let mut file = File::create(format!("{}/{}.json", INPUT_DIR, temp_name)).unwrap();

    let input_bytes = serde_json::to_string(&input).unwrap().into_bytes();
    file.write_all(&input_bytes).unwrap();
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
    let mixer_vec = match fs::read_to_string(Path::new(MIXER_MNEMONIC_PATH)) {
        Ok(string) => {
            let mnemonic = Mnemonic::parse(&string).unwrap();
            let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
            let xprv = xkey.into_xprv(network).unwrap();
            utils::init_client_wallet(network, &host, &vec![xprv.to_string()])
        }
        Err(e) => {
            eprintln!("Faild to read file {}: {}", MIXER_MNEMONIC_PATH, e);
            std::process::exit(1);
        }
    };
    let mixer = &mixer_vec[0];
    mixer.sync(noop_progress(), None).unwrap();
    println!(
        "Mixer {:?} has {:?}",
        mixer.get_address(AddressIndex::Peek(0)).unwrap(),
        mixer.get_balance().unwrap()
    );

    let psbt_inputs = Path::new(SERVER_INPUT_DIR)
        .read_dir()
        .map(|res| {
            res.map(|e| {
                e.and_then(|e| match fs::read_to_string(&e.path()) {
                    Ok(data) => {
                        let coinjoin_input: CoinJoinInput = serde_json::from_str(&data).unwrap();
                        Ok(coinjoin_input)
                    }
                    Err(e) => {
                        eprintln!("Faild to read file: {}", e);
                        std::process::exit(1);
                    }
                })
            })
        })
        .map(|resp| resp.collect::<std::result::Result<Vec<CoinJoinInput>, io::Error>>())
        .unwrap_or_else(|err| {
            eprintln!("Faild to read directory: {}", err);
            std::process::exit(1);
        })
        .unwrap_or_else(|err| {
            eprintln!("Faild to read file: {}", err);
            std::process::exit(1);
        });

    let output_addresses = Path::new(OUTPUT_DIR)
        .read_dir()
        .and_then(|dir| {
            dir.filter_map(|result| result.ok())
                .map(|e| match fs::read_to_string(&e.path()) {
                    Ok(data) => Ok(Address::from_str(&data).unwrap()),
                    Err(e) => {
                        eprintln!("Faild to read file: {}", e);
                        std::process::exit(1);
                    }
                })
                .collect::<io::Result<Vec<Address>>>()
        })
        .unwrap();

    let (psbt, _) = {
        let mut builder = mixer.build_tx();
        builder
            .fee_rate(bdk::FeeRate::from_sat_per_vb(10.0))
            .do_not_spend_change();

        for output_addres in output_addresses {
            builder.add_recipient(output_addres.script_pubkey(), 100_000_000);
        }

        for psbt_input in &psbt_inputs {
            // Register outpoint and psbt input
            // outpoint is pointing utxo which used in coinjoin tx
            // psbt_input is input of coinjoin tx which we are going to create
            let psbt_input_string: Vec<u8> = FromHex::from_hex(&psbt_input.psbt_input).unwrap();
            builder
                .add_foreign_utxo(
                    into_rust_bitcoin_output(&OutPoint::from_str(&psbt_input.outpoint).unwrap()),
                    deserialize::<Input>(&psbt_input_string).unwrap(),
                    32,
                )
                .unwrap(); // check about weight
        }
        builder.finish().unwrap()
    };

    let hex_psbt = serialize_hex(&psbt);
    println!("{:?}", hex_psbt);
    let mut file = std::fs::File::create(PSBT_PATH).unwrap();
    file.write_all(hex_psbt.as_bytes()).unwrap();

    Ok(HttpResponse::Ok().finish())
}

fn into_rust_bitcoin_output(out_point: &OutPoint) -> bitcoin::blockdata::transaction::OutPoint {
    bitcoin::blockdata::transaction::OutPoint {
        txid: out_point.txid,
        vout: out_point.vout,
    }
}
