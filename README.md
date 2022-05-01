# Overview
Implementing CoinJoin server and client process.
Try to generate CoinJoin transaction which has five inputs and five outputs.  

# Initial setup

Start regtest bitcoind and electrs
To stop bitcoind, run `make stop-bitcoind`
```shell
make run-servers
```

Fund alice wallet
```shell
bitcoin-cli --regtest createwallet "alice" false false "" false true
```
or
```shell
bitcoin-cli --regtest loadwallet "alice"
```

```shell
bitcoin-cli --regtest importdescriptors '[{ "desc": "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/*)#88ru8wxx", "timestamp":0 }]'
```
To get checksum of descriptor
```shell
bitcoin-cli --regtest getdescriptorinfo "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/*)"
```

```shell
bitcoin-cli --regtest generatetodescriptor 120 "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/0)"
```

Send to the others wallets from Alice
```shell
bitcoin-cli --regtest sendtoaddress bcrt1qwj7d4n048pj7xerwfnl2qmmnlk9ggjs7v0fqs7 20
bitcoin-cli --regtest sendtoaddress bcrt1qzwjn0h2zuw3r5ju9v44vn4fhdxmpg59sr5eydq 20
bitcoin-cli --regtest sendtoaddress bcrt1q83z6lkpsl6zr4ygnd20hmucxfshqw79e33jljw 20
bitcoin-cli --regtest sendtoaddress bcrt1qeqh35t057g0mr0mpyxhqc0ggxekm0ua9mrpv97 20
bitcoin-cli --regtest sendtoaddress bcrt1q7hv0uc8pun3u9vex4lxvf9tev8qxdn68pw6r30 20
```

If electrs is waiting for IBD, you need to mine a block.  
```shell
$ [electrs::daemon] waiting for 0 blocks to download (IBD)
```

```shell
bitcoin-cli --regtest generatetodescriptor 1 "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/0)"
```

# Run API server
```shell
HOST="127.0.0.1:50001" NETWORK="regtest" cargo watch -x run
```

## Prepare utxos and psbt input as client

This will dump utxos to ./data/client/utxos
```shell
curl localhost:8080/api/v1/utxo
```

This will dump hex psbt input to ./data/client/psbt_inputs
```shell
curl localhost:8080/api/v1/psbt-input
```

## Construct PSBT as server
This will dump unsigned PSBT as ./data/psbt.txt
```shell
curl localhost:8080/api/v1/psbt
```

For test net
```shell
HOST="ssl://blockstream.info:993" NETWORK="testnet" cargo run
HOST="ssl://electrum.blockstream.info:60002" NETWORK="testnet" cargo run
```

## Sign PSBT as client
```shell
cargo test sign_psbt -- --nocapture
```

# Client wallet by bdk-cli
```shell
bdk-cli wallet -s 127.0.0.1:50001 --descriptor "wpkh(tprv8ZgxMBicQKsPd5qWpCSXVYmARoGqNAKi8iGjdJdAWkLM33WDmEB264qK81i8f9G1WiSHcK4QRyNrFxjDMB5VqNK2z1JvkpaQxuZvN2X7tot/84'/1'/0'/0/*)" --change_descriptor "wpkh(tprv8ZgxMBicQKsPeaTaHASFZrywrLFR2hreAv8yjoc1GkcgM6MKJ3pZDDN3Kdsqtz9xaZj4asJiyhH7ATuW7FzrCg1amJcEcmU2VdYCk4eMjYC/84'/1'/0'/1/*)" --wallet bob sync
```

```shell
bdk-cli wallet -s 127.0.0.1:50001 --descriptor "wpkh(tprv8ZgxMBicQKsPdu9c5ujos5QUdGzUu3TkXWhTjTYiJ2SGEscdPALwZPXPn7fypuMi6eiD42YDV1qRTEdiV5DNaYLhHEijUn4Y7dAiJAHYCn3/84'/1'/0'/0/*)" --change_descriptor "wpkh(tprv8ZgxMBicQKsPdu9c5ujos5QUdGzUu3TkXWhTjTYiJ2SGEscdPALwZPXPn7fypuMi6eiD42YDV1qRTEdiV5DNaYLhHEijUn4Y7dAiJAHYCn3/84'/1'/0'/1/*)" --wallet carol sync
```

```shell
bdk-cli wallet -s 127.0.0.1:50001 --descriptor "wpkh(tprv8ZgxMBicQKsPeA6GrkH85xhTDfDz8oBbjqSnXJCXiohMEr1sxNmc7VwtiAWyrT9rNxWnNveaZ2MKJ9nAC1htJPvKkDZs7w8p6jHEtZWMk3J/84'/1'/0'/0/*)" --change_descriptor "wpkh(tprv8ZgxMBicQKsPeA6GrkH85xhTDfDz8oBbjqSnXJCXiohMEr1sxNmc7VwtiAWyrT9rNxWnNveaZ2MKJ9nAC1htJPvKkDZs7w8p6jHEtZWMk3J/84'/1'/0'/1/*)" --wallet dave sync
```

```shell
bdk-cli wallet -s 127.0.0.1:50001 --descriptor "wpkh(tprv8ZgxMBicQKsPeaTaHASFZrywrLFR2hreAv8yjoc1GkcgM6MKJ3pZDDN3Kdsqtz9xaZj4asJiyhH7ATuW7FzrCg1amJcEcmU2VdYCk4eMjYC/84'/1'/0'/0/*)" --change_descriptor "wpkh(tprv8ZgxMBicQKsPeaTaHASFZrywrLFR2hreAv8yjoc1GkcgM6MKJ3pZDDN3Kdsqtz9xaZj4asJiyhH7ATuW7FzrCg1amJcEcmU2VdYCk4eMjYC/84'/1'/0'/1/*)" --wallet eve sync
```

```shell
bdk-cli wallet -s 127.0.0.1:50001 --descriptor "wpkh(tprv8ZgxMBicQKsPeSeYKZhm4A2JDKunauLBtRP9HiDUb7frW8RNkzofRSESaLnkXL4m5ihADerbgFmLyWWJvzwU28HALbYEFqRrsNbpHFUaAoa/84'/1'/0'/0/*)" --change_descriptor "wpkh(tprv8ZgxMBicQKsPeSeYKZhm4A2JDKunauLBtRP9HiDUb7frW8RNkzofRSESaLnkXL4m5ihADerbgFmLyWWJvzwU28HALbYEFqRrsNbpHFUaAoa/84'/1'/0'/1/*)" --wallet frank sync
```
