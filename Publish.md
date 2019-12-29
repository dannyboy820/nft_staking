# Publishing Contracts

This is a quick overview of how to publish contracts in this repo, or any other
repo.

## Preparation

Ensure the `Cargo.toml` file in the repo is properly configured. In particular, you want to
choose a name starting with `cw-`, which will help a lot finding cosmwasm contracts when
searching on crates.io. For the first publication, you will probably want version `0.1.0`.
If you have tested this on a public net already and/or had an audit on the code,
you can start with `1.0.0`, but that should imply some level of stability and confidence.
You will want entries like the following in `Cargo.toml`:

```toml
name = "cw-escrow"
version = "0.1.0"
```

## Registry

You will need an account on [crates.io](https://crates.io) to publish a rust crate.
If you don't have one already, just click on "Log in with GitHub" in the top-right
to quickly set up a free account. Once inside, click on your username (top-right),
then "Account Settings". On the bottom, there is a section called "API Access".
If you don't have this set up already, create a new token and use `cargo login`
to set it up. This will now authenticate you with the `cargo` cli tool and allow
you to publish.

## Publishing

Once this is set up, make sure you commit the current state you want to publish.
Then try `cargo publish --dry-run`
