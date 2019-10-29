# Cosmwasm Examples

This repo is a collection of contracts built with the
[cosmwasm](https://github.com/confio/cosmwasm) framework.
Anyone building on cosmwasm is encouraged to submit their contracts
as a sub-project via a PR.

The organization is relatively simple. The top-level directory is just a placeholder
and has no real code. And we use workspaces to add multiple contracts below.
This allows us to compile all contracts with one command.

## Usage:

The following contracts are available for use. You can view the source code under `src`
and a precompiled wasm ready for deployment under `contract.wasm`. Take a look here:

## Development

### Starting a contract

If you want to add a contract, first fork this repo and create a branch for your PR.
I suggest setting it up via [cosmwasm-template](https://github.com/confio/cosmwasm-template):

``cargo generate --git https://github.com/confio/cosmwasm-template.git --name FOO`

Then make sure it is listen in `Cargo.toml`:

```toml
[workspaces]
members = ["FOO", "...."]
```

Once you add this, you can start writing your code and testing it.

### Preparing for merge

Before you merge the code, make sure it builds and passes all tests, both in the package,
and when calling it from the root packages `cargo wasm && cargo test`. This should
show your package is covered by the CI.

You should also prepare a compiled `contract.wasm` before each merge to master.
This is not enforced by the CI (a full build each commit), but should be tested
on merge. To validate:

```shell script
sha256sum contract.wasm
rm contract.wasm
docker run --rm -u $(id -u):$(id -g) -v $(pwd):/code confio/cosmwasm-opt:0.4.1
sha256sum contract.wasm
```

If the sha256 hash changes, then please commit the new version. And if the sha256 hash
changes without any code changes, then please submit an issue on [cosmwasm-opt](https://github.com/confio/cosmwasm-opt).

Once you pass these checks, please open a [PR on this repo](https://github.com/confio/cosmwasm-examples/pulls).