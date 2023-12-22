# build settings

## host

aarch64-apple-darwin

node v20.10.0

## aarch64-apple-darwin

```sh
cargo build --target aarch64-apple-darwin --release
```

## x86_64-unknown-linux-musl

reference [link](https://github.com/messense/homebrew-macos-cross-toolchains)

```sh
rustup target add x86_64-unknown-linux-musl
brew tap messense/macos-cross-toolchains
brew install x86_64-unknown-linux-musl
cargo build --target x86_64-unknown-linux-musl --release
```

## x86_64-pc-windows-msvc

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
cargo xwin build --target x86_64-pc-windows-msvc --release
```
