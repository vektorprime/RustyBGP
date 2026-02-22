RustyBGP is my first attempt at routing protocol written in, you guessed it, Rust.

What is implemented:

- Processing of the following BGP messages: Open, Keepalive, Updates
- Neighborship comes up
- Keeping the neighborship up
- Sending routes
- Receiving routes
- IPv4 Unicast Address Family
- Async via Tokio
- 2 byte and 4 byte ASN
- Resuming of neighbors after they go down
- Optional parameters for neighbors (capabilities like AS4, and other address families)
- Receiving and understanding (but not doing anything with) Notifications

What isn't implemented yet:
-  GUI
-  Best Path Calc.
-  Processing of BGP Notification
-  Handling of route refresh
-  Other address families
