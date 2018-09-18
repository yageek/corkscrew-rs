# corkscrew-rs

Quick and dirty port of [corkscrew](http://www.agroman.net/corkscrew/) in Rust.

## Installation

```sh
cargo install corkscrew-rs
```

## Usage with SSH

In your `~/.ssh/config`:

```
ProxyCommand /usr/local/bin/corkscrew-rs proxy.work.com 80 %h %p ~/.ssh/myauth
```
