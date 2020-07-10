# Voting

This is a simple voting contract. It creates a contract to manage token weighted polls,
where voters deposit native coins in order to vote.
Voters can withdraw their stake, but not while a poll they've participated in is still in progress.

Anyone can create a poll, and as the poll creator, only they are allowed to end/tally the poll.

This contract is mainly considered as a simple tutorial example.

As of v0.2.0, this was rebuilt from
[`cosmwasm-template`](https://github.com/confio/cosmwasm-template),
which is the recommended way to create any contracts.

## Using this project

If you want to get aquainted more with this contratc, you should check out
[Developing](./Developing.md), which explains more on how to run tests and develop code.
[Publishing](./Publishing.md) contains useful information on how to publish your contract
to the world, once you are ready to deploy it on a running blockchain. And
[Importing](./Importing.md) contains information about pulling in other contracts or crates
that have been published.

But more than anything, there is an [online tutorial](https://www.cosmwasm.com/docs/getting-started/intro),
which leads you step-by-step on how to modify this particular contract.
