cargo b --release --target wasm32-unknown-unknown
cp ./target/wasm32-unknown-unknown/release/game.wasm ../bruh/www/release/game.wasm
cp config.json ../bruh/www/config.json
rm -rf ../bruh/www/art
cp -r ./art ../bruh/www/
