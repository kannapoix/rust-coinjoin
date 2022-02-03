# How to create coinjoin transaction
This code generate coinjoin transaction whici has two inputs and two outputs.  
First, we create two trasactions. The one contain input which Alice has priv key and the other tx contain input which Bob has it.    
Then coinjoiner combine two trasnactions into one coinjoined trasaction.  
Lastly, each Alice and Bob sign the trasaction by their own prive key.  

# Initial setup

```
// Start regtest bitcoind and electrs
// To stop bitcoind, run "make stop-bitcoind"
make run-servers

// Fund alice wallet
bitcoin-cli --regtest createwallet "alice" false false "" false true
// or  bitcoin-cli --regtest loadwallet "alice"
bitcoin-cli --regtest importdescriptors '[{ "desc": "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/*)#88ru8wxx", "timestamp":0 }]'
// To get checksum of descriptor
bitcoin-cli --regtest getdescriptorinfo "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/*)"

bitcoin-cli --regtest generatetodescriptor 110 "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/0)"

// Send to the others wallets from Alice
bitcoin-cli --regtest sendtoaddress bcrt1qwj7d4n048pj7xerwfnl2qmmnlk9ggjs7v0fqs7 20
bitcoin-cli --regtest sendtoaddress bcrt1qzwjn0h2zuw3r5ju9v44vn4fhdxmpg59sr5eydq 20
bitcoin-cli --regtest sendtoaddress bcrt1q83z6lkpsl6zr4ygnd20hmucxfshqw79e33jljw 20
bitcoin-cli --regtest sendtoaddress bcrt1qeqh35t057g0mr0mpyxhqc0ggxekm0ua9mrpv97 20
bitcoin-cli --regtest sendtoaddress bcrt1q7hv0uc8pun3u9vex4lxvf9tev8qxdn68pw6r30 20
```

If electrs is waiting for IBD, you need to mine a block.  
```
$ [electrs::daemon] waiting for 0 blocks to download (IBD)
```

```
bitcoin-cli --regtest generatetodescriptor 1 "wpkh(tprv8ZgxMBicQKsPcwmRJonpDgQecfz4yQ29EzGoJE8gdo22yhWZHJVdWcatkKTy28CqGxnfuyZmaVeehVb52RPJVc1qrs8dVR6uQvcZwWdcX5w/84h/1h/0h/0/0)"
```

# Run
Use regtest server on your local.
```
HOST="127.0.0.1:50001" NETWORK="regtest" cargo run
HOST="ssl://blockstream.info:993" NETWORK="testnet" cargo run
HOST="ssl://electrum.blockstream.info:60002" NETWORK="testnet" cargo run
```
