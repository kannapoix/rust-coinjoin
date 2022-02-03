~/bitcoind/0.21.0-bin/bin/bitcoind -daemon -chain=regtest
electrs -vv --timestamp --db-dir /tmp/electrs-db \
--electrum-rpc-addr="127.0.0.1:50001" --network=regtest \
--cookie-file=$HOME/.bitcoin/regtest/.cookie \
--txid-limit 0

# to stop bitcoind
~/bitcoind/0.21.0-bin/bin/bitcoin-cli --regtest stop

# mine to first pool address
bitcoin-cli --regtest createwallet pool
bitcoin-cli --regtest loadwallet pool
bitcoin-cli --regtest -rpcwallet=pool getbalance
bitcoin-cli --regtest -rpcwallet=pool getnewaddress
bitcoin-cli --regtest generatetoaddress 101 bcrt1qhrppumgv4xfg5frhdr3prp4eu6nat3f4pmg6sa

# fund alice wallet
bitcoin-cli --regtest loadwallet pool
bitcoin-cli --regtest -rpcwallet=pool sendtoaddress "bcrt1qge5h7zy2y57jjcvznandp0naq9luppa7mpfs53" 1
bitcoin-cli --regtest -rpcwallet=pool sendtoaddress "bcrt1qx7pwj9htxa7y9e7sd0d7rwecmj5fdkacn4w6lt" 1
