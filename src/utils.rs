use bdk::bitcoin;
use bdk::blockchain::ElectrumBlockchain;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::wallet::Wallet;
use bdk::Error;

type InnerWallet = Wallet<ElectrumBlockchain, MemoryDatabase>;

pub fn init_client_wallet(
    network: bitcoin::Network,
    electrum_endpoint: &str,
    clients: &Vec<String>,
) -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
    clients
        .iter()
        .map(|client| {
            let descriptors = prepare_descriptor(client);
            return generate_wallet(&descriptors[0], &descriptors[1], network, electrum_endpoint)
                .unwrap();
        })
        .collect()
}

fn prepare_descriptor(base: &str) -> [String; 2] {
    let descriptor = format!("wpkh({}/84'/1'/0'/0/*)", base);
    let change_descriptor = format!("wpkh({}/84'/1'/0'/1/*)", base);
    return [descriptor, change_descriptor];
}

fn generate_wallet(
    descriptor: &str,
    change_descriptor: &str,
    network: bitcoin::Network,
    electrum_endpoint: &str,
) -> std::result::Result<InnerWallet, Error> {
    let client = Client::new(electrum_endpoint).unwrap();
    Wallet::new(
        descriptor,
        Some(change_descriptor),
        network,
        MemoryDatabase::default(),
        ElectrumBlockchain::from(client),
    )
}
