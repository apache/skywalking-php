# Kafka reporter

By default, the configuration option `skywalking_agent.reporter_type` is `grpc`, means that the skywalking agent will report the traces, metrics, logs etc. to SkyWalking OAP Server by gPRC protocol.

At the same time, SkyWalking also supports kafka-fetcher, so you can report traces, metrics, logs, etc. by kafka.

But the skywalking agent does not compile the `kafka-reporter` feature by default, you need to enable the it.

## Steps

1. Compile the skywalking agent with feature `kafka-reporter`.

   For pecl:

   ```shell
   pecl install skywalking_agent
   ```

   Enable the kafka reporter interactively:

   ```txt
   68 source files, building
   running: phpize
   Configuring for:
   PHP Api Version:         20220829
   Zend Module Api No:      20220829
   Zend Extension Api No:   420220829
   enable cargo debug? [no] : 
   enable kafka reporter? [no] : yes
   ```

   Or, build from sources:

   ```shell
   phpize
   ./configure --enable-kafka-reporter
   make
   make install
   ```

2. Config `php.ini`.

   Switch to use kafka reporter.

   ```ini
   [skywalking_agent]
   extension = skywalking_agent.so
   skywalking_agent.reporter_type = kafka
   skywalking_agent.kafka_bootstrap_servers = 127.0.0.1:9092,127.0.0.2:9092,127.0.0.3:9092
   ```

   If you want to custom the kafka reporter properties, you can specify it by JSON format:

   ```ini
   skywalking_agent.kafka_producer_config = {"delivery.timeout.ms": "12000"}
   ```
