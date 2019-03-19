<img width="40%" height="40%" src="https://mech-lang.org/img/logo.png">

Mech is a language for developing **data-driven**, **reactive** systems like animations, games, and robots. It makes **composing**, **transforming**, and **distributing** data easy, allowing you to focus on the essential complexity of your problem. 

Read about progress on our [blog](https://mech-lang.org/blog/), follow us on Twitter [@MechLang](https://twitter.com/MechLang), or join the mailing list: [talk@mech-lang.org](https://mech-lang.org/page/community/).

# Mech Server

Mech Server is the way most users will work with Mech. It hosts a websocket server that accepts connections from a Mech notebook. The main contribution of this module is the notion of a `Program`, which is a network of Mech cores.

## Contents

- client - defines a protocol for accepting messages from websocket clients, and a `ClientHandler` that implements this protocol.
- program - defines a `Program`, which starts a Mech core on an OS thread; and a `Program Runner`, which marshalls messages to and from that thread.

## License

Apache 2.0