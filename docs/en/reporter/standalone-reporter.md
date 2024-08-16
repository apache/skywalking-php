# Standalone reporter

When the reporter type is `grpc` or `kafka`, the `skywalking_agent` extension forks a child process during
the extension initialization phase to act as a worker process for sending data to the SkyWalking OAP server
or Kafka.

However, this approach has some limitations, such as:

1. It cannot be used with the `php-fpm` daemon mode.
2. Multiple worker processes can be redundant when there are several `php-fpm` processes on the instance.

To address these issues, `skywalking_agent` introduces a new reporter type: `standalone`.

With the `standalone` reporter type, the `skywalking_agent` extension no longer forks a child process.
Instead, the user needs to manually start an independent worker process.

## Steps

1. Compile the standalone `skywalking-php-worker` binary:

   ```shell
   cargo build -p skywalking-php-worker --bin skywalking-php-worker --all-features --release
   ```

2. Run `skywalking-php-worker`:

   Assuming the socket file path is `/tmp/skywalking-php-worker.sock` and the SkyWalking OAP server address is `127.0.0.1:11800`, the command is:

   ```shell
   ./target/release/skywalking-php-worker -s /tmp/skywalking-php-worker.sock grpc --server-addr 127.0.0.1:11800
   ```

   For additional parameters, refer to `./target/release/skywalking-php-worker --help`.

3. Configure `php.ini`:

   ```ini
   [skywalking_agent]
   extension = skywalking_agent.so
   skywalking_agent.reporter_type = standalone
   skywalking_agent.standalone_socket_path = /tmp/skywalking-php-worker.sock
   ```
