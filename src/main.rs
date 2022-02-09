use std::env;
use std::fs;
use std::path::Path;
use core::str::FromStr;
use serde_json;

use bdk::wallet::{Wallet, AddressIndex};
use bdk::database::{MemoryDatabase};
use bdk::blockchain::{noop_progress, ElectrumBlockchain};
use bdk::electrum_client::Client;
use bdk::Error;
use bdk::bitcoin::Network;
use bdk::bitcoin::consensus::encode::serialize_hex;
use bdk::bitcoin;
use bdk::bitcoin::OutPoint;
use bdk::bitcoin::secp256k1::{Secp256k1};
use bdk::keys::bip39::Mnemonic;
use bdk::keys::{DerivableKey, ExtendedKey};

type InnerWallet = Wallet<ElectrumBlockchain, MemoryDatabase>;
type PSBT = bdk::bitcoin::util::psbt::PartiallySignedTransaction;

const MNEMONIC_DIR: &str = "./data/client/mnemonic";
const MIXER_MNEMONIC_PATH: &str = "./data/mixer/mnemonic/alice.mnemonic";

#[derive(Debug, Clone)]
struct OutPut {
    out_point: OutPoint,
    tx_out: bitcoin::blockdata::transaction::TxOut,
}

// #[derive(Debug, Copy, Clone)]
// struct OutPoint {
//     txid: bitcoin::hash_types::Txid,
//     vout: u32,
// }

#[derive(Debug)]
struct CoinJoinInput {
    prev_output: OutPut,
    input: bitcoin::util::psbt::Input,
}

fn main() {
    const ENV_HOST: &str = "HOST";
    const ENV_NETWORK: &str = "NETWORK";

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
    
    // Client side work
    // May be it is good to write as test
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


    // pubkey wallets の変わりにファイルから読み込んだ json から local uxto を生成すればいい気がする
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

    // サーバーはPSBT input を作成するために、クライアントから受け取る pubkey から wallet を生成する
    let mut pubkey_clients:Vec<String> = Vec::new();
    for mnemonic in mnemonics.iter() {
        // TODO: pubkey をクライアントからもらったものをつかう
        let xkey_for_pubkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
        let xpub = xkey_for_pubkey.into_xpub(network, &Secp256k1::new());
        pubkey_clients.push(xpub.to_string());
    }
    let pubkey_wallets = init_client_pubkey_wallet(network, &host, &pubkey_clients);

    // let outpoints = list_outpoints(&pubkey_wallets);
    // let mut outpoints = Vec::new();
    // for utxo in &utxos {
    //     // outpoints.push(bdk::bitcoin::OutPoint::from_str(utxo.outpoint));
    //     outpoints.push(utxo.outpoint);
    // }
    // let outputs = list_outputs(outpoints, &pubkey_wallets);
    let mut outputs = Vec::new();
    for utxo in utxos.clone().into_iter() {
        outputs.push(OutPut {
            out_point: utxo.outpoint,
            tx_out: utxo.txout,
        });
    }

    let mut psbt_inputs: Vec<bitcoin::util::psbt::Input> = Vec::new();

    for wallet in pubkey_wallets.iter() {
        // get_psbt_input はクライアントが行うことで、実際にはこのような値をサーバーは受け取ることになる
        // クライアントからは pubkey をもらい、サーバーで descriptor -> wallet を生成して psbt input を生成できる
        for i in 0..5 {
            match wallet.get_psbt_input(utxos[i].clone(), None, false) {
                Ok(input) => {
                    println!("Ok {:?}", input);
                    psbt_inputs.push(input);
                },
                Err(err) => {println!("Erro {:?}", err)},
            }
        }
        // psbt_inputs.push(wallet.get_psbt_input(wallet.list_unspent().unwrap()[0].clone(), None, false).unwrap())
        // psbt_inputs.push(wallet.get_psbt_input(utxos[0].clone(), None, false))
    }

    let input_pairs = outputs.iter().zip(psbt_inputs.iter());
    let coinjoins: Vec<CoinJoinInput> = input_pairs.map(|(output, input)| CoinJoinInput{prev_output: output.clone(), input: input.clone()}).collect();

    // // Responsible for tumbler
    let (psbt, _) = {
        let mut builder = mixer.build_tx();
        builder
            .fee_rate(bdk::FeeRate::from_sat_per_vb(10.0))
            .do_not_spend_change();

        for _ in 0..5 {
            builder.add_recipient(mixer.get_address(AddressIndex::New).unwrap().script_pubkey(), 5_000);
        }

        for coinjoin in &coinjoins {
            // bitcoin::blockdata::transaction::OutPoint を PSBT input にいれるものとおもわれる
            builder.add_foreign_utxo(into_rust_bitcoin_output(&coinjoin.prev_output.out_point), coinjoin.input.clone(), 32).unwrap();// check about weight
        }
        builder.finish().unwrap()
    };

    // 署名はクライアントが行う
    // let psbts = list_signed_txs(psbt, &wallets);
    // match merge_psbts(psbts).pop() {
    //     Some(psbt) => {
    //         println!("Finalized PSBT {:?}", &serialize_hex(&psbt));
    //         println!("Finalized tx extracted from PSBT {:?}", &serialize_hex(&psbt.extract_tx()));
    //     },
    //     None => println!("{:?}", "Can not get first item.")
    // };
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
        let _ = wallet.sign(&mut psbt, bdk::SignOptions::default()).unwrap();
        psbts.push(psbt.clone())
    };
    psbts
}

fn list_outputs(outpoints: Vec<OutPoint>, wallets: &Vec<InnerWallet>) -> Vec<OutPut> {
    let mut outputs = Vec::new();
    for (index, outpoint) in outpoints.into_iter().enumerate() {
        outputs.push(OutPut {
            out_point: outpoint,
            tx_out: wallets[index].list_unspent().unwrap()[0].txout.clone(),
        });
    }
    outputs
}

fn list_outpoints(wallets: &Vec<InnerWallet>) -> Vec<OutPoint> {
    // TODO: get OutPoint from client
    // txID and vout are necessary
    let mut outpoints = Vec::new();
    for wallet in wallets.iter() {
        let utxo = wallet.list_unspent().unwrap();
        outpoints.push(OutPoint {
            txid: utxo[0].outpoint.txid,
            vout: utxo[0].outpoint.vout,
        });
    }
    outpoints
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
    let descriptor = format!("wpkh({})", "0375254c039ce50308b3a9a89f08843d7bfc2dbf058e62e26d47be92e5a392d7ed");
    // Addr is not working
    // let descriptor = format!("addr({})", "bcrt1qfxuuk7m6vmrnc6h6ta7fu8g56qn3qh6pg03nah");
    let change_descriptor = format!("wpkh({})", base);
    return [descriptor, change_descriptor];
}

fn into_rust_bitcoin_output(out_point: &OutPoint) -> bitcoin::blockdata::transaction::OutPoint {
    bitcoin::blockdata::transaction::OutPoint{ txid: out_point.txid, vout: out_point.vout }
}

fn generate_wallet(descriptor: &str, change_descriptor: &str, network: bitcoin::Network, electrum_endpoint: &str) -> Result<InnerWallet, Error> {
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

    use std::fs::File;
    use std::io::prelude::*;

    #[test]
    fn test_add() {
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
        let wallets = init_client_wallet(Network::Regtest, "127.0.0.1:50001", &clients);

        for (i, wallet) in wallets.iter().enumerate() {
            wallet.sync(noop_progress(), None).unwrap();
            println!("wallet {:?} has {:?}", wallet.get_address(AddressIndex::Peek(0)).unwrap(), wallet.get_balance().unwrap());
            // まず決め打ちで取得
            let local_utxo = &wallet.list_unspent().unwrap()[0];
            // Outpoint は含まれている
            let json = serde_json::to_vec(&local_utxo).unwrap(); // use to_vec instead of to_string

            // pubkey が必要
            // いや local utxo json にして dump してそれ読み取れば一旦いいのでは？

            let mut file = File::create(format!("./data/client/utxos/{}.json", i)).unwrap();
            file.write_all(&json).unwrap();
        }

        // Outpoint と pubkey を dump する。実際にはサーバーに post する

    }
}
