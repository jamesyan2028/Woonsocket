# Project 0: Woonsocket

Welcome to your first project in CS1675!

In this project, you will learn how to measure performance and understand how different workloads affect the performance of network system.
You will implement a client-server busy work application.
The client will generate work requests and transmit them to the server.
The server will receive these requests, perfom the requested work, and return responses.
The client will receive those responses.

Beyond implementation, the overall goal is to analyze how different types of load generation affect the results we observe, under different workload types.
You should evaluate both closed-loop and open-loop request generation.
For open-loop request generation, you should evaluate both constant-distribution request arrivals and Poisson arrivals.

## Deliverables

You will submit a report in the style of a Jupyter Notebook (or similar) with
**at least** the following:

1. A graph showing that your implementation has correct performance behavior.
2. Throughput-latency graphs for median, 95th, and 99th percentile latency (these can be on the same graph) across different workloads.
3. A written analysis, using data the graphs show, of the performance characteristics of the request generation strategies.

While this is the minimum requirement, your report should describe and justify your design decisions and provide evidence that the performance characteristics you discuss are represented correctly in whichever way you need to.
While your implementation will of course inform the contents of your report, the report is the primary deliverable.

### Grading

We will assign a grade following an interactive 1-1 meeting between you and one of the course staff in which you describe and defend the contents of your report. The idea is to emulate a meeting between you and a skeptical user, where you use your report's contents as evidence that your findings are correct. You should expect questions of the form, "Why does this graph look the way it does?"

Projects in 1675, including this one, are graded on a coarse 5-point scale, which we assign as follows:

1. Implementation correctness
2. Distinguishing attempted load from offered load (including correctly generating the offered load)
3. Distinguishing offered load from achieved load
4. Demonstrating a difference between request generation strategies
5. Demonstrating the correct relationship between throughput and latency

Successful reports will generally include headings for each of the points.
Remember that if you lose points on a project, you can earn up to 3 points per project back at the subsequent grading meeting by fixing any problems in your report that arise.

## Implementation

To provide you with maximum implementation flexibility, there is no starter or skeleton code for this project.
Instead, we provide two pieces of code:

1. A Rust crate implementing synthetic work types. You must use this Rust crate in your server implementation to perfom the synthetic work, and your implementation must work with all the types of work in the `Work` enum. You can specify the dependency on this crate in your `Cargo.toml` with `woonsocket-work = { git = "https://github.com/cs1675/woonsocket-work" }`
2. A Python script and Dockerfile to run your code in Docker containers on your machine. This script tests that your implementation exposes the command-line argument interface that the course VMs expect (see [CLI Behavior](#cli-behavior)).

### Hint

The Python script (and grading server) are pre-configured to iterate through several attempted load values. 
A correct implementation will be able to *offer* all these attempted loads, but may not be able to *achieve* all the resulting offered loads.

### Required Behaviors

#### Overall Structure

To correctly perform the synthetic work, you must implement logic to, in a loop until the number of seconds given in `--runtime-secs`:

1. (client) Decide when to send a request (three ways: closed-loop, open-loop with constant arrivals, open-loop with Poisson arrivals)
2. (client) Create a request object corresponding to the command-line arguments
3. (client) Serialize the request object to bytes
4. (client) Send those bytes to the server.
5. (server) Read request bytes from the client.
6. (server) Deserialize the request bytes into a request object.
7. (server) Do the work with `perform()`
8. (server) Create a response object, including the response specified in the return value of `perform()` (if this value is `None`, then the response is empty).
9. (server) Serialize the response object to bytes. Remember that this object could be variably sized.
10. (server) Send the response bytes to the client.
11. (client) Receive the response bytes.
12. (client) Deserialize the response bytes into a response object.
13. (client) Log the required data about the request/response pair.

#### CLI Behavior

> To reduce the tedious parts of this project, we've updated the `woonsocket-work` crate to provide optional command-line argument parsing functionality.

Your **client** should provide the command line options in the following way:
```
your-woonsocket <REQUIRED_OPTIONS> <COMMAND> <MODE_SPECIFIC_OPTIONS>
```

`<REQUIRED_OPTIONS>`:
- `-r, --runtime-secs <RUNTIME_SECS>`: Provided hint to the client for when to finish writing logs and exit before the runner script terminates this process
- `--ip <IP> `: Server IP.
- `-p, --port <PORT> `: Port that the server is listening on.
- `-w, --work <WORK> `: The worktype that will be sent to the server from the client.
- `-o, --outpath <OUTPATH>`: The directory in which to store the results. Only files written to this directory will be preserved; the runner script will delete all other files.

`<COMMAND>`: Must be `closed-loop` or `open-loop`.

`<MODE_SPECIFIC_OPTIONS>`:
- If `<COMMAND>` is `closed-loop`:
  - `-n, --num-threads <NUM_THREADS>`: The number of clients (threads) your closed loop generator will start.
- If `<COMMAND>` is `open-loop`:
  - `--interval-us <INTERVAL_US>`: Mean interval between request arrivals in microseconds.
  - `--kind <KIND>`: Must be `constant` or `poisson`.

<details><summary>Example Usage</summary>

```
Usage: your-woonsocket-client --runtime-secs <RUNTIME_SECS> --ip <IP> --port <PORT> --work <WORK> --outpath <OUTPATH> <COMMAND>

Commands:
  closed-loop  
  open-loop    

Options:
  -r, --runtime-secs <RUNTIME_SECS>  Provided hint for when to finish writing logs and exit before the runner script terminates this process
      --ip <IP>                      
  -p, --port <PORT>                  
  -w, --work <WORK>                  
  -o, --outpath <OUTPATH>            Only files written to this directory will be preserved; the runner script will delete all other files
```

```
Usage: your-woonsocket-client --runtime-secs <RUNTIME_SECS> --ip <IP> --port <PORT> --work <WORK> --outpath <OUTPATH> closed-loop --num-threads <NUM_THREADS>

Options:
  -n, --num-threads <NUM_THREADS>  
```

```
Usage: your-woonsocket-client --runtime-secs <RUNTIME_SECS> --ip <IP> --port <PORT> --work <WORK> --outpath <OUTPATH> open-loop --interval-us <INTERVAL_US> --kind <KIND>

Options:
      --interval-us <INTERVAL_US>  Mean interval between request arrivals in microseconds
      --kind <KIND>                [possible values: constant, poisson]
```
</details>

Your **server** should provide the following command line options:

- `-p, --port <PORT>`: Port the server is listening for connections.
- `-r, --runtime-secs <RUNTIME_SECS>`: Provided hint to the server for when to finish writing logs and exit before the runner script terminates this process
- `-o, --outpath <OUTPATH>`: The directory in which to store the results. Only files written to this directory will be preserved; the runner script will delete all other files.

<details><summary>Example Usage</summary>

```
Usage: your-woonsocket-server --port <PORT> --runtime-secs <RUNTIME_SECS> --outpath <OUTPATH>

Options:
  -p, --port <PORT>                  
  -r, --runtime-secs <RUNTIME_SECS>  Provided hint for when to finish writing logs and exit before the runner script terminates this process
  -o, --outpath <OUTPATH>            Only files written to this directory will be preserved; the runner script will delete all other files
```

</details>

#### Work and Work Communications

There are four types of `Work` that the client can request (see the `woonsocket-work` crate):

There are four types of work:

- `Immediate`: Zero overhead. There is no actual computation and the server
  completes this immediately.
- `Const`: The server pauses for a constant amount of time.
- `Poisson`: The server pauses for an amount of time controlled by the Poisson
  distribution.
- `Payload`: The server generates a payload to send back to the client.

Since you are the one writing both the client and the server, you can decide what the client `Request` and server `Response` look like.

#### Performance Data to Record

We care about both latency and throughput performance, which means for each request we want to record an end-to-end latency of handling the request (i.e from when client generates the request to when the request is completed and response is received by the client).

An example `LatencyRecord` for a client request might look like the following:

```rust
struct LatencyRecord {
    pub latency: u64,
    pub send_timestamp: u64,
    pub server_processing_time: u64,
    pub recv_timestamp: u64,
}
```

### Exiting

Both the client and server must exit after the number of seconds provided in `--runtime-secs`. Your implementation should write any data to stdout, stderr, or other files before exiting. Otherwise, our script will terminate your process after a timeout and you might not have access to your logs.

### Third-Party Crates

Your implementation can use third-party Rust crates for basic functionality such as command-line argument functionality, serialization, or logging, but you *may not* use third-party crates that implement project components. Using `std` is fine.

Here is a list of third-party crates you do not need permission to use:

- [clap](https://crates.io/crates/clap)
- Serialization-focused crates and [`serde`](https://serde.rs). [This benchmark](https://david.kolo.ski/rust_serialization_benchmark/) is a good rundown of available options.
- [tracing](https://crates.io/crates/tracing) and [tracing-subscriber](https://crates.io/crates/tracing-subscriber)
- [quanta](https://crates.io/crates/quanta)
- [rand](https://crates.io/crates/rand) and [rand_distr](https://crates.io/crates/rand_distr)

If there's a specific crate you're not sure about and want permission to use, please ask on EdStem.

### Tips

- Structure your code in modules - many components can be shared across clients and server. 
- Separate closed- and open- loop clients code. 
- Think about and design the protocol (everything) before implementation.

## Running

### Testing Locally

The Python script and Dockerfile we provide allow you to test your implementation locally. This will help you understand the command-line interface your implementation needs to support. The course VMs will use the same command line arguments as this Python script, and if the Python script works in the Docker container the Dockerfile describes, it will work on the course VMs (otherwise, the course VM runner has a bug, which we will do our best to fix or mitigate).

### Course VMs

To ensure consistency between students' performance, we require that your final report use only data from runs on the course VMs.
To help you debug your implementation's performance, we also provide `perf` output.
The submission site to use the course VMs is
[https://cs1675.cs.brown.edu/submit](https://cs1675.cs.brown.edu/submit).
You should submit your GitHub repository and select a submission commit using this site, and you can click a button to join the run queue.

The output on this site will provide links to download the following log and output files:

1. stdout and stderr for client and server
2. perf.data for client and server
3. the client's output that you will generate

1675's only use of the submission site is to run your code and provide the output log files.
