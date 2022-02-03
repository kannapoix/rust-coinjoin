use std::env;
use std::fs;
use std::path::Path;

use bdk::wallet::{Wallet, AddressIndex};
use bdk::database::{MemoryDatabase};
use bdk::blockchain::{noop_progress, ElectrumBlockchain};
use bdk::electrum_client::Client;
use bdk::Error;
use bdk::bitcoin::Network;
use bdk::bitcoin::consensus::encode::serialize_hex;
use bdk::bitcoin;
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

#[derive(Debug, Copy, Clone)]
struct OutPoint {
    txid: bitcoin::hash_types::Txid,
    vout: u32,
}

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

    let mixer_vec  = match fs::read_to_string(Path::new(MIXER_MNEMONIC_PATH))  {
        Ok(string) => {
            let mnemonic = Mnemonic::parse(&string).unwrap();
            let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
            let xprv = xkey.into_xprv(network).unwrap();
            init_client_wallet(network, &host, vec![xprv.to_string()])
        },
        Err(e) => {
            eprintln!("Faild to read file {}: {}", MIXER_MNEMONIC_PATH, e);
            std::process::exit(1);
        }
    };
    let mixer = &mixer_vec[0];
    mixer.sync(noop_progress(), None).unwrap();
    println!("Mixer {:?} has {:?}", mixer.get_address(AddressIndex::Peek(0)).unwrap(), mixer.get_balance().unwrap());

    let mut clients:Vec<String> = Vec::new();
    for mnemonic in mnemonics.iter() {
        let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
        let xprv = xkey.into_xprv(network).unwrap();
        clients.push(xprv.to_string());
    }
    let wallets = init_client_wallet(network, &host, clients);

    for wallet in wallets.iter() {
        wallet.sync(noop_progress(), None).unwrap();
        println!("{:?} has {:?}",wallet.get_address(AddressIndex::Peek(0)).unwrap(), wallet.get_balance().unwrap());
    }

    let outpoints = list_outpoints(&wallets);
    let outputs = list_outputs(outpoints, &wallets);

    let mut psbt_inputs: Vec<bitcoin::util::psbt::Input> = Vec::new();

    for wallet in wallets.iter() {
        psbt_inputs.push(wallet.get_psbt_input(wallet.list_unspent().unwrap()[0].clone(), None, false).unwrap())
    }
    
    let input_pairs = outputs.iter().zip(psbt_inputs.iter());
    let coinjoins: Vec<CoinJoinInput> = input_pairs.map(|(output, input)| CoinJoinInput{prev_output: output.clone(), input: input.clone()}).collect();

    // Responsible for tumbler
    let (psbt, _) = {
        let mut builder = mixer.build_tx();
        builder
            .fee_rate(bdk::FeeRate::from_sat_per_vb(10.0))
            .do_not_spend_change();

        for _ in 0..5 {
            builder.add_recipient(mixer.get_address(AddressIndex::New).unwrap().script_pubkey(), 5_000);
        }

        for coinjoin in &coinjoins {
            builder.add_foreign_utxo(into_rust_bitcoin_output(&coinjoin.prev_output.out_point), coinjoin.input.clone(), 32).unwrap();// check about weight
        }
        builder.finish().unwrap()
    };
    let psbts = list_signed_txs(psbt, &wallets);
    match merge_psbts(psbts).pop() {
        Some(psbt) => {
            println!("Finalized PSBT {:?}", &serialize_hex(&psbt));
            println!("Finalized tx extracted from PSBT {:?}", &serialize_hex(&psbt.extract_tx()));
        },
        None => println!("{:?}", "Can not get first item.")
    };
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

fn init_client_wallet(network: bitcoin::Network, electrum_endpoint: &str, clients: Vec<String>) -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
    clients.iter().map( |client| {
        let descriptors = prepare_descriptor(client);
        return generate_wallet(&descriptors[0], &descriptors[1], network, electrum_endpoint).unwrap();
    }).collect()
}

fn prepare_descriptor(base: &str) -> [String;2] {
    let descriptor = format!("wpkh({}/84'/1'/0'/0/*)", base);
    let change_descriptor = format!("wpkh({}/84'/1'/0'/1/*)", base);
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
