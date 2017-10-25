# Concurr: Distributed and Concurrent Command Execution, in Rust

> This project is dual licensed under MIT and Apache 2.0.

Originally inspired by the GNU Parallel project, and my previous re-implementation of the project
in Rust, Concurr is a distributed and concurrent command-line job server & client architecture for
generating and executing commands in parallel on any number of systems. At it's core, Concurr
uses Tokio to perform asynchronous network I/O. The client works similarly to GNU Parallel, but
there are key differences for the sake of simplicity in operation.

## The Client

The client is responsible for parsing arguments, connecting to nodes and obtaining slot counts,
submitting a command to each node, distributing inputs to each slot on each connected node,
collecting responses from those slots when they complete, and requesting new inputs from a shared
buffer. Simple stuff. Syntax is to be very similar to GNU Parallel, but there are some differences.

### Example

```sh
concurr 'echo job {#} on slot {%}: {}' : arg1 arg2 arg3 arg4
concurr 'echo job {#} on slot {%}: {}' :: file1 file2 file3
concurr 'echo {}' < input_file
cat file | concurr 'echo {}'
```

### How The Client Works

## The Server

### How The Server Works

The service works by listening for a number of possible instructions that can be supplied. A
command instruction will tell the server to create a new command with a pool of threads that will
listen for inputs, henceforth named as slots. A delete instruction can be used to delete commands
from the server, thus causing all threads to exit after completing their tasks. Input instructions
will supply inputs to commands, which are designated by an integer ID, and also specify the ID of
the job, which may be useful to the client knowing which input received what results.

Effectively, a client can send a command to a server, which will spawn as many threads as there are
cores in the system; initializing an instance of the Ion shell on each slot. These slots have
shared access to an input buffer, output buffer, kill switch, and corresponding counter
to count the number of threads that have exited. These are all wrapped up together in a unit.

When an input is received from a client, that input is matched to a unit and then placed onto an
input buffer that is collectively owned by the threads that are attached to that command. When a
slot grabs that input, it will perform a fork, capture the standard output and error of the fork,
execute the command within an embedded Ion instance attached to that slot on the child, and then
wait for the child to exit before placing the exit status, job ID, and file descriptors containing
the standard output and error onto an output buffer.

The connection that submitted the input will have been waiting for a result that matches the ID of
the job that was submitted, and upon seeing that job, will immediately encode a response with the
results.

### Example

1. The following command, assigned ID 0, is sent to the server: `echo {#}: {}`
2. The server has four cores, and creates four slots for that command.
3. Some inputs are submitted to the command with ID 0:
  - inp 0 1 one
  - inp 0 2 two
  - inp 0 3 three
  - inp 0 4 four
4. Slots concurrently grab inputs from the queue, process them, and return their outputs:
  - 1: one
  - 2: two
  - 3: three
  - 4: four
5. The exit status, job ID, standard out, and standard error are returned to the client.
