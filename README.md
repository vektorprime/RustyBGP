RustyBGP is my first attempt at routing protocol written in, you guessed it, Rust.

What is implemented:

- Processing of the following BGP messages: Open, Keepalive, Updates
- Neighborship comes up
- Keeping the neighborship up
- Sending routes
- Receiving routes
- IPv4 Unicast Address Family
- Async via Tokio

What isn't implemented yet:

-  GUI
-  Best Path Calc.
-  Optional Parameters for neighbor.
-  Processing of the following BGP messages: Notification
-  Handling of route refresh
