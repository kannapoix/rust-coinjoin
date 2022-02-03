BITCOIND_PATH := ~/.bitcoin/regtest

.PHONY:
run-bitcoind:
	bitcoind -chain=regtest -rpccookiefile=".cookie" -daemon

.PHONY:
run-electrs:
	electrs -vv --timestamp --db-dir /tmp/electrs-db --electrum-rpc-addr="127.0.0.1:50001" --network=regtest --cookie-file=$$HOME/.bitcoin/regtest/.cookie --index-lookup-limit 0

.PHONY: run-servers
run-servers: run-bitcoind run-electrs 

.PHONY:
stop-bitcoind:
	bitcoin-cli -regtest stop

.PHONY:
reset-bitcoind:
	rm -R $(BITCOIND_PATH)/indexes
	rm -R $(BITCOIND_PATH)/blocks
	rm -R $(BITCOIND_PATH)/chainstate
