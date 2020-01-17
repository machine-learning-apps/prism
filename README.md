# Prism, http microbatcher

Prism is open-source high-performance batching microservice. It excells in ML inference scenarios, where batching of inference calls can provice big improvements in peroformance (especially using GPU).

It will hold requests until either batch is filled or timeout pass, then make single request to backend service with aggregated requests. Backend serive will provide map of responses corresponding to requests, and Prism will forward these responses to related users.

# Contribute

After you setup your typical Rust environment, compiling and running is as simple as

```
cargo run
```

from Prism directory
