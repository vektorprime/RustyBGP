RustyBGP is my first attempt at routing protocol written in, you guessed it, Rust.

What is implemented:

Processing of BGP Open messages
- Neighborship comes up
- Keepalives sent

What isn't implemented yet:
- Exchanging routes (Update messages)
- Handling of multiple peers
