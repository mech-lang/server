<img width="40%" height="40%" src="https://mechlang.net/img/logo.png">

Mech is a language for developing **data-driven**, **reactive** systems like animations, games, and robots. It makes **composing**, **transforming**, and **distributing** data easy, allowing you to focus on the essential complexity of your problem. 

Read about progress on our [blog](https://mechlang.net/blog/), follow us on Twitter [@MechLang](https://twitter.com/MechLang), or join the mailing list: [talk@mechlang.net](https://mechlang.net/page/community/).

# Mech Server

Mech Server is the way most users will work with Mech. It hosts a websocket server that accepts connections from a Mech notebook. The main contribution of this module is the notion of a `Program`, which is essentially a network of Mech cores. The program can start up any number of cores on any number of threads, either remotely or locally.

## Build

```
cargo build --bin server --release
```

## License

Apache 2.0