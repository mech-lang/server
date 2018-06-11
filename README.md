<img width="40%" height="40%" src="https://mechlang.net/img/logo.png">

---

# Mech Server

Mech Server is the way most users will work with Mech. It hosts a websocket server that accepts connections from a Mech notebook. The main contribution of this module is the notion of a `Program`, which is essentially a network of Mech cores. The program can start up any number of cores on any number of threads, either remotely or locally.

## Build

```
cargo build --bin server --release
```

## License

Apache 2.0